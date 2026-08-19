// Worker 入口：/api/bookmarks 读写 R2 书签数据；其余路由交静态资源。
// 页面两张，同一份数据：/ = 书签页（全库列表）；/123 = 导航页（folder `123` 的卡片视图）。
// 自定义域 123.yigegongjiang.com 指向同一 Worker，host 命中即渲染导航页（路径无语义）。
//
// 认证 = 双层：① Cloudflare Access 在边缘按 Google 登录网关；② 本 Worker 再校验
// Access 注入的 JWT（Cf-Access-Jwt-Assertion），堵住绕过边缘（如直连 workers.dev）的口子。
// run_worker_first=true 使**所有**请求先经此校验，页面与 API 都不裸奔。
// CF_ACCESS_TEAM_DOMAIN / CF_ACCESS_AUD 缺任一则跳过 ②，仅靠 ①（便于本地 dev / 未配场景）。

const R2_KEY = "bookmarks.json"; // 与 CLI sync 固定 key 对齐（data-model 顶层 {version,sources}）
const BOOKMARKS_VERSION = 4; // data-model schema v4（含 deleted 墓碑）；与 CLI CURRENT_VERSION 同步
const BOOKMARKS_MAX_BYTES = 8 * 1024 * 1024; // 真实数据 ~0.9 MB；留足增长余量，同时挡住失控写入
const MAX_SOURCES = 100;
const MAX_BOOKMARKS = 50000;
const MAX_TIMESTAMP = 4102444800000; // 2100-01-01Z，超出即视为脏数据（Date 越界会抛）
const NAV_HOST = "123.yigegongjiang.com";

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
      return handleBookmarks(request, env);
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

// ---- 书签数据（R2 bookmarks.json；CLI 与 web 双向写，靠 ETag CAS 收敛） ----

/// GET → 全量库（原样透传 R2 对象）+ `ETag` 响应头；对象缺失 = 空库、无 ETag。
/// PUT → 服务端白名单重建后整份覆写；`If-Match` 走 R2 条件写（onlyIf.etagMatches），失配 409。
///
/// 缺 `If-Match` 时只允许「对象尚不存在」这一种情况（head 命中即 409）——否则一个不带条件的
/// PUT 会静默盖掉另一端刚写的内容，CAS 就成了摆设。
async function handleBookmarks(request, env) {
  if (request.method === "GET") {
    const obj = await env.BOOKMARKS.get(R2_KEY);
    // 首次 sync 前对象不存在 → 返回空库，页面渲染空态而非 500
    if (!obj) {
      return jsonResponse(JSON.stringify({ version: BOOKMARKS_VERSION, sources: {} }));
    }
    return new Response(obj.body, {
      headers: {
        "content-type": "application/json; charset=utf-8",
        "cache-control": "no-store", // 双向同步，永远取最新写入
        etag: obj.httpEtag,
      },
    });
  }
  if (request.method === "PUT") {
    const raw = await request.text();
    if (raw.length > BOOKMARKS_MAX_BYTES) return textError(413, "payload too large");
    let doc;
    try {
      doc = JSON.parse(raw);
    } catch {
      return textError(400, "invalid JSON");
    }
    const clean = sanitizeStore(doc);
    if (typeof clean === "string") return textError(400, clean);

    const ifMatch = bareEtag(request.headers.get("If-Match"));
    if (!ifMatch && (await env.BOOKMARKS.head(R2_KEY))) {
      return textError(409, "If-Match required: the object already exists");
    }
    const res = await env.BOOKMARKS.put(R2_KEY, JSON.stringify(clean, null, 2) + "\n", {
      httpMetadata: { contentType: "application/json" },
      ...(ifMatch ? { onlyIf: { etagMatches: ifMatch } } : {}),
    });
    if (!res) return textError(409, "etag mismatch"); // 条件写失配 → 客户端重新拉取再合并
    return new Response(JSON.stringify({ etag: res.etag }), {
      headers: {
        "content-type": "application/json; charset=utf-8",
        "cache-control": "no-store",
        etag: res.httpEtag,
      },
    });
  }
  return textError(405, "method not allowed");
}

/// HTTP 头里的 etag 带引号、可能带弱校验前缀 `W/`；R2 的 `etagMatches` 要裸值。
function bareEtag(value) {
  if (!value) return null;
  const bare = value.trim().replace(/^W\//, "").replace(/^"|"$/g, "");
  return bare || null;
}

/// 校验并按白名单重建书签库；合法返回重建后的对象，非法返回错误消息字符串。
///
/// 取舍：**结构错误才拒绝，字段值一律强转**。字段值上没有长度上限（整体已被
/// BOOKMARKS_MAX_BYTES 兜住），因为一条超长 excerpt 让整次 PUT 失败 = 两端永远同步不上，
/// 代价远大于收益。URL 不校验协议——真实数据里存在非 http(s) 条目。
function sanitizeStore(doc) {
  if (!doc || typeof doc !== "object" || Array.isArray(doc)) return "doc must be an object";
  if (doc.version !== BOOKMARKS_VERSION) return `unsupported version (want ${BOOKMARKS_VERSION})`;
  if (!doc.sources || typeof doc.sources !== "object" || Array.isArray(doc.sources)) {
    return "sources must be an object";
  }
  const names = Object.keys(doc.sources);
  if (names.length > MAX_SOURCES) return "too many sources";

  const sources = {};
  const seenIds = new Set();
  for (const name of names) {
    const list = doc.sources[name];
    if (!Array.isArray(list)) return `source ${JSON.stringify(name)} must be an array`;
    const source = name.trim() || "default"; // 与 CLI normalize_source 一致
    const target = (sources[source] ||= []);
    for (const b of list) {
      if (!b || typeof b !== "object" || Array.isArray(b)) return "bookmark must be an object";
      // id 是合并的唯一键：缺失或重复都会让 LWW 失去意义，属结构错误，必须拒绝
      if (!Number.isFinite(b.id)) return "bookmark id must be a number";
      const id = Math.floor(b.id);
      if (seenIds.has(id)) return `duplicate bookmark id ${id}`;
      seenIds.add(id);
      if (seenIds.size > MAX_BOOKMARKS) return "too many bookmarks";

      const created = clampTimestamp(b.created);
      const updated = clampTimestamp(b.updated);
      const lastVisited = clampTimestamp(b.last_visited);
      target.push({
        id,
        title: text(b.title),
        url: text(b.url),
        excerpt: text(b.excerpt),
        note: text(b.note),
        folder: text(b.folder),
        deleted: b.deleted === true,
        created,
        created_jst: jst(created),
        updated,
        updated_jst: jst(updated),
        last_visited: lastVisited,
        last_visited_jst: lastVisited === 0 ? "" : jst(lastVisited),
      });
    }
  }
  // 空组不落盘（与 CLI normalize 一致）；键排序令输出稳定，避免无意义的 etag 抖动
  const ordered = {};
  for (const source of Object.keys(sources).sort()) {
    if (sources[source].length) ordered[source] = sources[source];
  }
  return { version: BOOKMARKS_VERSION, sources: ordered };
}

function text(value) {
  return typeof value === "string" ? value : "";
}

function clampTimestamp(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.min(Math.max(Math.floor(value), 0), MAX_TIMESTAMP);
}

/// epoch 毫秒 → `YYYY-MM-DD HH:MM:SS+09:00`。JST 恒为 UTC+9（日本无夏令时），
/// 故「+9h 后按 UTC 格式化」与 CLI 的 timeutil.rs 逐字节等价。派生值一律服务端重算，
/// 不信任客户端传来的 *_jst——否则 web 写入可能留下与数字主字段矛盾的可读串。
function jst(ms) {
  return new Date(ms + 9 * 3600 * 1000).toISOString().slice(0, 19).replace("T", " ") + "+09:00";
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
