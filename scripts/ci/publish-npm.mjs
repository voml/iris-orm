/**
 * GitHub Actions: publish real npm packages (not placeholder stubs).
 *
 * - tag vX.Y.Z → version X.Y.Z
 * - Idempotent: skip when version already on registry
 * - No NPM_TOKEN; OIDC Trusted Publisher (permissions.id-token: write)
 * - Contract: file=publish-npm.yml env=NPM_PUBLISH repo=voml/iris-orm
 *
 * Prereq: @yydb/iris native + wasm built before publish; CI workflow runs build.
 */

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

/** @type {{ dir: string, publishName?: string, prebuild?: string }[]} */
const PACKAGES = [
    { dir: 'projects/iris.ts/iris', prebuild: 'pnpm napi:build && pnpm wasm:build' },
    { dir: 'projects/iris.ts/iris-skills' },
    { dir: 'projects/iris.ts/iris-win32-x64', publishName: '@yydb/iris-win32-x64' },
    { dir: 'projects/iris.ts/iris-linux-x64', publishName: '@yydb/iris-linux-x64' },
    {
        dir: 'projects/iris.ts/iris-unknown-wasm32',
        publishName: '@yydb/iris-unknown-wasm32',
        prebuild: 'pnpm wasm:build',
    },
];

function fail(msg) {
    console.error(`ci-publish-npm: ${msg}`);
    process.exit(1);
}

function run(cmd, args, opts = {}) {
    const r = spawnSync(cmd, args, {
        cwd: opts.cwd ?? ROOT,
        encoding: 'utf8',
        shell: process.platform === 'win32',
        env: opts.env ?? process.env,
        stdio: opts.stdio ?? 'pipe',
    });
    return {
        status: r.status ?? 1,
        stdout: String(r.stdout ?? '').trim(),
        stderr: String(r.stderr ?? '').trim(),
    };
}

function resolveVersion() {
    const fromArg = process.argv.find((a) => a.startsWith('--version='))?.slice('--version='.length);
    if (fromArg) return fromArg.replace(/^v/, '');
    const ref = process.env.GITHUB_REF ?? '';
    const m = ref.match(/^refs\/tags\/(?:placeholder-)?v?(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)$/);
    if (m) return m[1];
    fail('need --version=X.Y.Z or GITHUB_REF=refs/tags/vX.Y.Z');
}

function readJson(p) {
    return JSON.parse(fs.readFileSync(p, 'utf8'));
}

function writeJson(p, obj) {
    fs.writeFileSync(p, `${JSON.stringify(obj, null, 2)}\n`);
}

function copyTree(src, dest, filter) {
    fs.mkdirSync(dest, { recursive: true });
    for (const name of fs.readdirSync(src)) {
        if (name === 'node_modules' || name === '.git') continue;
        const from = path.join(src, name);
        const to = path.join(dest, name);
        const st = fs.statSync(from);
        if (st.isDirectory()) {
            if (filter && !filter(from, true)) continue;
            copyTree(from, to, filter);
        } else {
            if (filter && !filter(from, false)) continue;
            fs.copyFileSync(from, to);
        }
    }
}

/**
 * @param {Record<string, string>} deps
 * @param {string} version
 */
function rewriteWorkspaceDeps(deps, version) {
    if (!deps) return deps;
    /** @type {Record<string, string>} */
    const out = {};
    for (const [k, v] of Object.entries(deps)) {
        if (typeof v === 'string' && (v.startsWith('workspace:') || v === '*')) {
            out[k] = version;
        } else {
            out[k] = v;
        }
    }
    return out;
}

function rewriteDepsField(pkg, version) {
    for (const field of ['dependencies', 'optionalDependencies', 'peerDependencies']) {
        if (pkg[field]) pkg[field] = rewriteWorkspaceDeps(pkg[field], version);
    }
    return pkg;
}

function isAlreadyPublished(blob) {
    return /cannot publish over existing|EPUBLISHCONFLICT|previously published versions|version already exists|cannot publish.*same version|you cannot publish over/i.test(
        blob,
    );
}

function isAuthFailure(blob) {
    return /ENEEDAUTH|Unable to authenticate|not authorized|OIDC|trusted publisher|two-factor|need to be logged|login|identity token|do not have permission to access it|Access token expired or revoked/i.test(
        blob,
    );
}

function isMissingPackage(blob) {
    if (isAuthFailure(blob)) return false;
    return /Package not found|does not exist on the registry|cannot publish.*before creating|This package has not been created|is not in this registry/i.test(
        blob,
    );
}

function versionExists(name, version) {
    const r = run('npm', ['view', `${name}@${version}`, 'version']);
    return r.status === 0 && r.stdout === version;
}

/**
 * @param {string} stagingDir
 * @param {string} name
 * @param {string} version
 * @returns {'published'|'exists'|'auth'|'missing'|'other'}
 */
function npmPublish(stagingDir, name, version) {
    const args = ['publish', '--access', 'public'];
    console.log(`\n=== ${name}@${version} npm ${args.join(' ')} ===`);
    const r = run('npm', args, { cwd: stagingDir });
    if (r.stdout) process.stdout.write(`${r.stdout}\n`);
    if (r.stderr) process.stderr.write(`${r.stderr}\n`);
    const blob = `${r.stdout}\n${r.stderr}`;
    if (r.status === 0) return 'published';
    if (isAlreadyPublished(blob) || versionExists(name, version)) return 'exists';
    if (isAuthFailure(blob)) return 'auth';
    if (isMissingPackage(blob)) return 'missing';
    if (versionExists(name, version)) return 'exists';
    console.error(blob.slice(0, 1200));
    return 'other';
}

/**
 * @param {string} version
 */
function publishJs(version) {
    let published = 0;
    let skipped = 0;

    for (const spec of PACKAGES) {
        const abs = path.join(ROOT, spec.dir);
        if (!fs.existsSync(abs)) fail(`missing package dir ${spec.dir}`);

        if (spec.prebuild && process.env.IRIS_SKIP_PREBUILD !== '1') {
            console.log(`\n--- prebuild: ${spec.prebuild} ---`);
            const parts = spec.prebuild.trim().split(/\s+/);
            const cmd = parts[0];
            const args = parts.slice(1);
            const br = run(cmd, args, { stdio: 'inherit' });
            if (br.status !== 0) fail(`prebuild failed for ${spec.dir}`);
        } else if (spec.prebuild && process.env.IRIS_SKIP_PREBUILD === '1') {
            console.log(`\n--- prebuild skipped (IRIS_SKIP_PREBUILD) for ${spec.dir} ---`);
        }

        const raw = readJson(path.join(abs, 'package.json'));
        const name = spec.publishName ?? raw.name;
        if (!name) fail(`no name for ${spec.dir}`);

        if (versionExists(name, version)) {
            console.log(` ✓ ${name}@${version} already on registry — skip`);
            skipped += 1;
            continue;
        }

        const stage = path.join(os.tmpdir(), `iris-pub-js-${name.replace(/[/@]/g, '-')}-${version}`);
        fs.rmSync(stage, { recursive: true, force: true });

        const files = Array.isArray(raw.files) && raw.files.length ? raw.files : null;
        fs.mkdirSync(stage, { recursive: true });
        if (files) {
            for (const f of files) {
                const from = path.join(abs, f);
                if (!fs.existsSync(from)) continue;
                const st = fs.statSync(from);
                const to = path.join(stage, f);
                if (st.isDirectory()) copyTree(from, to);
                else {
                    fs.mkdirSync(path.dirname(to), { recursive: true });
                    fs.copyFileSync(from, to);
                }
            }
            for (const extra of ['package.json', 'README.md', 'LICENSE', 'License.md', 'bin']) {
                const from = path.join(abs, extra);
                if (!fs.existsSync(from)) continue;
                const to = path.join(stage, extra);
                if (fs.statSync(from).isDirectory()) copyTree(from, to);
                else fs.copyFileSync(from, to);
            }
        } else {
            copyTree(abs, stage, (p) => {
                const rel = path.relative(abs, p);
                if (rel.includes('node_modules') || rel.includes('tests')) return false;
                return true;
            });
        }

        const pkg = rewriteDepsField({ ...raw }, version);
        pkg.name = name;
        pkg.version = version;
        delete pkg.private;
        pkg.publishConfig = { ...(pkg.publishConfig ?? {}), access: 'public' };
        if (!pkg.repository) {
            pkg.repository = {
                type: 'git',
                url: 'git+https://github.com/voml/iris-orm.git',
                directory: spec.dir.replace(/\\/g, '/'),
            };
        }
        delete pkg.devDependencies;
        writeJson(path.join(stage, 'package.json'), pkg);

        if (!fs.existsSync(path.join(stage, 'README.md'))) {
            fs.writeFileSync(path.join(stage, 'README.md'), `# ${name}\n\nIris ORM package ${version}.\n`);
        }

        const outcome = npmPublish(stage, name, version);
        if (outcome === 'published') published += 1;
        else if (outcome === 'exists') {
            console.log(` ✓ ${name}@${version} already on registry — skip`);
            skipped += 1;
        } else if (outcome === 'auth') {
            fail(`OIDC/auth failed for ${name}. Add Trusted Publisher: file=publish-npm.yml env=NPM_PUBLISH repo=voml/iris-orm`);
        } else if (outcome === 'missing') {
            fail(
                `${name} is not on the registry yet. Create the name first via placeholder stubs (pnpm placeholder:publish), then retry real publish.`,
            );
        } else fail(`publish failed for ${name}`);
    }
    return { published, skipped };
}

const version = resolveVersion();
console.log(`ci-publish-npm: version=${version}`);
console.log(` GITHUB_REF=${process.env.GITHUB_REF ?? '(none)'}`);
console.log(' Trusted Publisher contract: publish-npm.yml + env NPM_PUBLISH\n');

delete process.env.NODE_AUTH_TOKEN;
delete process.env.NPM_TOKEN;

const js = publishJs(version);

console.log(`\nci-publish-npm: done (published=${js.published} skipped=${js.skipped})`);
