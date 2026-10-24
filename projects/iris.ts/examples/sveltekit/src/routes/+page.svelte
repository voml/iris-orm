<main style="font-family: system-ui; padding: 2rem">
    <h1>@yydb/iris + SvelteKit</h1>
    <p>Schema: <code>author: &amp;User</code> on <code>Post</code>.</p>
    <ul>
        <li><code>GET /api/posts</code> — map through <code>x.author.user_name</code></li>
        <li><code>GET /api/posts?author=ada</code> — filter on the reference</li>
        <li><code>POST /api/posts</code> — insert with <code>author_user_id</code></li>
    </ul>
</main>
