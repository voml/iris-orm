export default function Home() {
    return (
        <main style={{ fontFamily: "system-ui", padding: "2rem" }}>
            <h1>@yydb/iris + Next.js</h1>
            <p>
                Schema uses <code>author: &amp;User</code> on <code>Post</code>. Try:
            </p>
            <ul>
                <li><code>GET /api/posts</code> — map through <code>x.author.user_name</code></li>
                <li><code>GET /api/posts?author=ada</code> — filter on the reference</li>
                <li><code>POST /api/posts</code> — insert with FK <code>author_user_id</code></li>
            </ul>
        </main>
    );
}
