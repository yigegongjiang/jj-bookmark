// Worker 入口：/api/bookmarks 读 R2 数据；/api/nav 读写导航页数据；其余路由交静态资源。
// 页面两张：/ = 只读 preview page；/123 = 个人导航页（hao123 式，web 即唯一 CRUD 入口）。
// 自定义域 123.yigegongjiang.com 指向同一 Worker，host 命中即渲染导航页（路径无语义）。
//
// 认证 = 双层：① Cloudflare Access 在边缘按 Google 登录网关；② 本 Worker 再校验
// Access 注入的 JWT（Cf-Access-Jwt-Assertion），堵住绕过边缘（如直连 workers.dev）的口子。
// run_worker_first=true 使**所有**请求先经此校验，页面与 API 都不裸奔。
// CF_ACCESS_TEAM_DOMAIN / CF_ACCESS_AUD 缺任一则跳过 ②，仅靠 ①（便于本地 dev / 未配场景）。

const R2_KEY = "bookmarks.json"; // 与 CLI push 固定 key 对齐（data-model 顶层 {version,bookmarks}）
const NAV_KEY = "nav.json"; // 导航页数据（同 bucket；仅经 /api/nav 读写，无本地副本）
const NAV_HOST = "123.yigegongjiang.com";
const NAV_MAX_BYTES = 512 * 1024;

export default {
  async fetch(request, env) {
    // 123 域未被 Access 应用覆盖时边缘不注入 JWT（无从登录），302 到主域同页走既有登录网关；
    // 人类在 Access 应用补上该 hostname 后请求自带 JWT，自动切回本域直出，无需改代码。
    const reqUrl = new URL(request.url);
    if (reqUrl.hostname === NAV_HOST && !getAccessToken(request)) {
      return Response.redirect("https://jj-bookmark.yigegongjiang.com/123", 302);
    }

    const denied = await verifyAccess(request, env);
    if (denied) return denied;

    const url = new URL(request.url);
    if (url.pathname === "/api/bookmarks") {
      const obj = await env.BOOKMARKS.get(R2_KEY);
      // 首次 push 前对象不存在 → 返回空库，页面渲染空态而非 500
      if (!obj) {
        return jsonResponse(JSON.stringify({ version: 3, sources: {} }));
      }
      return new Response(obj.body, {
        headers: {
          "content-type": "application/json; charset=utf-8",
          "cache-control": "no-store", // 单向同步，永远取最新 push
        },
      });
    }
    if (url.pathname === "/api/nav") {
      return handleNav(request, env);
    }

    // 导航域：任意非 API 路径都渲染导航页
    if (url.hostname === NAV_HOST) {
      url.pathname = "/123";
      return env.ASSETS.fetch(new Request(url, request));
    }
    // 非 API：静态资源（/ → public/index.html，/123 → public/123.html）
    return env.ASSETS.fetch(request);
  },
};

// ---- 导航页数据（R2 nav.json，页面即唯一数据源） ----

/// GET → { etag, data }（对象缺失 = 空库 + etag:null）；
/// PUT → 服务端字段白名单重建后整份覆写；If-Match 走 R2 条件写（onlyIf.etagMatches），
///        失配 409（防两个 tab 相互覆盖），成功回 { etag } 供客户端接续。
async function handleNav(request, env) {
  if (request.method === "GET") {
    const obj = await env.BOOKMARKS.get(NAV_KEY);
    if (!obj) {
      return jsonResponse(JSON.stringify({ etag: null, data: { version: 2, groups: [], links: [] } }));
    }
    const data = await obj.text();
    return jsonResponse(`{"etag":${JSON.stringify(obj.etag)},"data":${data}}`);
  }
  if (request.method === "PUT") {
    const raw = await request.text();
    if (raw.length > NAV_MAX_BYTES) return textError(413, "payload too large");
    let doc;
    try {
      doc = JSON.parse(raw);
    } catch {
      return textError(400, "invalid JSON");
    }
    const clean = sanitizeNav(doc);
    if (typeof clean === "string") return textError(400, clean);

    const ifMatch = request.headers.get("If-Match");
    const res = await env.BOOKMARKS.put(NAV_KEY, JSON.stringify(clean, null, 2), {
      httpMetadata: { contentType: "application/json" },
      ...(ifMatch ? { onlyIf: { etagMatches: ifMatch } } : {}),
    });
    if (!res) return textError(409, "etag mismatch"); // 条件写失配 → 客户端重载最新数据
    return jsonResponse(JSON.stringify({ etag: res.etag }));
  }
  return textError(405, "method not allowed");
}

/// 校验并按白名单重建导航数据；合法返回重建后的对象，非法返回错误消息字符串。
/// v2 结构：links 扁平（顺序 = 手动排序），group 内联在链接上；groups 仅记分组展示顺序。
/// 只收 v2 —— 页面读到 v1（{groups:[{name,links}]}）时在客户端迁移后再写回。
function sanitizeNav(doc) {
  if (!doc || typeof doc !== "object" || Array.isArray(doc)) return "doc must be an object";
  if (doc.version !== 2) return "unsupported version";
  if (!Array.isArray(doc.groups) || doc.groups.length > 200) return "invalid groups";
  if (!Array.isArray(doc.links) || doc.links.length > 1000) return "invalid links";
  const groups = [];
  for (const g of doc.groups) {
    if (typeof g !== "string" || !g || g.length > 100) return "invalid group name";
    if (!groups.includes(g)) groups.push(g);
  }
  const links = [];
  for (const l of doc.links) {
    if (!l || typeof l !== "object") return "invalid link";
    if (typeof l.name !== "string" || l.name.length > 200) return "invalid link name";
    if (typeof l.url !== "string" || l.url.length > 2048 || !/^https?:\/\//i.test(l.url)) {
      return "invalid link url";
    }
    if (typeof l.group !== "string" || l.group.length > 100) return "invalid link group";
    if (typeof l.color !== "string" || !/^#[0-9a-f]{6}$/i.test(l.color)) return "invalid link color";
    if (!Number.isFinite(l.createdAt) || l.createdAt < 0) return "invalid link createdAt";
    if (l.group && !groups.includes(l.group)) groups.push(l.group);
    links.push({
      name: l.name,
      url: l.url,
      group: l.group,
      color: l.color.toLowerCase(),
      createdAt: Math.floor(l.createdAt),
    });
  }
  return { version: 2, groups: groups.filter((g) => links.some((l) => l.group === g)), links };
}

function textError(status, msg) {
  return new Response(msg + "\n", {
    status,
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}

function jsonResponse(body) {
  return new Response(body, {
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  });
}

// ---- Cloudflare Access JWT 校验 ----

/// 返回 null = 放行；返回 Response = 拒绝（403）。
async function verifyAccess(request, env) {
  const team = (env.CF_ACCESS_TEAM_DOMAIN || "").replace(/\/+$/, "");
  const aud = env.CF_ACCESS_AUD || "";
  if (!team || !aud) return null; // 未配置 → 交给边缘 Access（本地 dev / 首次部署）

  const token = getAccessToken(request);
  if (!token) return deny("missing Access token");
  try {
    const ok = await verifyJwt(token, team, aud);
    return ok ? null : deny("invalid Access token");
  } catch {
    return deny("Access token verification failed");
  }
}

function deny(msg) {
  return new Response(`Forbidden: ${msg}\n`, {
    status: 403,
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}

/// Access 把 JWT 放在 Cf-Access-Jwt-Assertion 头；浏览器直连时也可能只在 CF_Authorization cookie。
function getAccessToken(request) {
  const header = request.headers.get("Cf-Access-Jwt-Assertion");
  if (header) return header;
  const cookie = request.headers.get("Cookie") || "";
  const m = cookie.match(/(?:^|;\s*)CF_Authorization=([^;]+)/);
  return m ? m[1] : null;
}

/// 校验 RS256 签名 + iss/aud/exp/nbf 声明。返回 payload（有效）或 null。
async function verifyJwt(token, teamDomain, aud) {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  const [h, p, s] = parts;

  const header = JSON.parse(b64urlToString(h));
  const payload = JSON.parse(b64urlToString(p));

  const now = Math.floor(Date.now() / 1000);
  if (payload.iss !== teamDomain) return null;
  const auds = Array.isArray(payload.aud) ? payload.aud : [payload.aud];
  if (!auds.includes(aud)) return null;
  if (typeof payload.exp === "number" && now >= payload.exp) return null;
  if (typeof payload.nbf === "number" && now < payload.nbf) return null;

  const jwks = await fetchJwks(teamDomain);
  const jwk = jwks.keys?.find((k) => k.kid === header.kid);
  if (!jwk) return null;

  const key = await crypto.subtle.importKey(
    "jwk",
    jwk,
    { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" },
    false,
    ["verify"]
  );
  const data = new TextEncoder().encode(`${h}.${p}`);
  const valid = await crypto.subtle.verify("RSASSA-PKCS1-v1_5", key, b64urlToBytes(s), data);
  return valid ? payload : null;
}

/// 取 Access 公钥集；交给 Cloudflare 边缘缓存（1h），避免每请求回源。
async function fetchJwks(teamDomain) {
  const res = await fetch(`${teamDomain}/cdn-cgi/access/certs`, {
    cf: { cacheTtl: 3600, cacheEverything: true },
  });
  if (!res.ok) throw new Error(`certs fetch failed: ${res.status}`);
  return res.json();
}

function b64urlToBytes(s) {
  s = s.replace(/-/g, "+").replace(/_/g, "/");
  const pad = s.length % 4 ? 4 - (s.length % 4) : 0;
  const bin = atob(s + "=".repeat(pad));
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function b64urlToString(s) {
  return new TextDecoder().decode(b64urlToBytes(s));
}
