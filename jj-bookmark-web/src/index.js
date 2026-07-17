// Worker 入口：/api/bookmarks 读 R2 数据；其余路由交静态资源（preview page）。
//
// 认证 = 双层：① Cloudflare Access 在边缘按 Google 登录网关；② 本 Worker 再校验
// Access 注入的 JWT（Cf-Access-Jwt-Assertion），堵住绕过边缘（如直连 workers.dev）的口子。
// run_worker_first=true 使**所有**请求先经此校验，页面与 API 都不裸奔。
// CF_ACCESS_TEAM_DOMAIN / CF_ACCESS_AUD 缺任一则跳过 ②，仅靠 ①（便于本地 dev / 未配场景）。

const R2_KEY = "bookmarks.json"; // 与 CLI push 固定 key 对齐（data-model 顶层 {version,bookmarks}）

export default {
  async fetch(request, env) {
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

    // 非 API：静态资源（/ → public/index.html）
    return env.ASSETS.fetch(request);
  },
};

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
