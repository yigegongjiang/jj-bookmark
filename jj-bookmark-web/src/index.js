// Worker 入口：/api/bookmarks 读 R2 数据；其余路由交静态资源（preview page）。
// 认证：由 Cloudflare Access 在边缘完成（Google 登录），到达 Worker 的请求已鉴权，
// 故此处不再做 JWT 校验（single source of trust = Access；见 README 安全说明）。

const R2_KEY = "bookmarks.json"; // 与 CLI push 默认 key 对齐（data-model 顶层 {version,bookmarks}）

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (url.pathname === "/api/bookmarks") {
      const obj = await env.BOOKMARKS.get(R2_KEY);
      // 首次 push 前对象不存在 → 返回空库，页面渲染空态而非 500
      if (!obj) {
        return jsonResponse(JSON.stringify({ version: 1, bookmarks: [] }));
      }
      // 透传 R2 原字节（CLI 上传的 pretty JSON），只补响应头
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
