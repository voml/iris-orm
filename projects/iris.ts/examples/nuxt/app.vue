<template>
    <main style="font-family: system-ui; padding: 2rem">
        <h1>@yydb/iris + Nuxt</h1>
        <p>Schema: <code>author: &amp;User</code> on <code>Post</code>.</p>
        <ul>
            <li><code>GET /api/posts</code> — project <code>x.author.user_name</code></li>
            <li><code>GET /api/posts?author=ada</code> — filter through the reference</li>
            <li><code>POST /api/posts</code> — insert with <code>author_user_id</code></li>
        </ul>
    </main>
</template>
