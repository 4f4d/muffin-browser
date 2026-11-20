import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const gallery = process.argv[2] === 'gallery';
const outputDirectory = resolve('dist', gallery ? 'gallery' : '.');
const htmlPath = resolve(outputDirectory, gallery ? 'gallery.html' : 'index.html');
let html = await readFile(htmlPath, 'utf8');

html = await inlineAssets(html, /<script\b[^>]*\bsrc="([^"]+)"[^>]*><\/script>/g, async (assetPath) => {
  const content = await readFile(resolve(outputDirectory, assetPath.replace(/^\.\//, '').replace(/^\//, '')), 'utf8');
  return `<script type="module">${content}</script>`;
});

html = await inlineAssets(html, /<link\b[^>]*\bhref="([^"]+\.css)"[^>]*>/g, async (assetPath) => {
  const content = await readFile(resolve(outputDirectory, assetPath.replace(/^\.\//, '').replace(/^\//, '')), 'utf8');
  return `<style>${content}</style>`;
});

const scriptHashes = [];
const styleHashes = [];
for (const match of html.matchAll(/<script type="module">([\s\S]*?)<\/script>/g)) scriptHashes.push(hashValue(match[1]));
for (const match of html.matchAll(/<style>([\s\S]*?)<\/style>/g)) styleHashes.push(hashValue(match[1]));
const csp = `default-src 'none'; script-src ${scriptHashes.map(quoteHash).join(' ')}; style-src ${styleHashes.map(quoteHash).join(' ')}; img-src data:; font-src data:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'`;
const charsetTag = /<meta\s+charset="[^"]+"\s*\/?\s*>/i;
if (!charsetTag.test(html)) throw new Error('HTML entry must declare its character encoding');
html = html.replace(charsetTag, (tag) => `${tag}\n    <meta http-equiv="Content-Security-Policy" content="${csp}" />`);

if (/<(?:script|link|img|source)\b[^>]*\b(?:src|href)=/i.test(html)) {
  throw new Error('Bundle still references an external asset');
}
if (/<(?:[a-z][a-z0-9-]*)\b[^>]*\bstyle=/i.test(html)) {
  throw new Error('Bundle uses inline style attributes outside the CSP policy');
}
const cspPosition = html.indexOf('http-equiv="Content-Security-Policy"');
const executablePosition = Math.min(...[html.indexOf('<script'), html.indexOf('<style')].filter((position) => position >= 0));
if (cspPosition < 0 || cspPosition > executablePosition) throw new Error('CSP must precede inline styles and scripts');
for (const content of [...scriptContents(), ...styleContents()]) {
  if (!csp.includes(quoteHash(hashValue(content)))) throw new Error('Inline asset is missing its CSP hash');
}
await writeFile(htmlPath, html);

async function inlineAssets(markup, pattern, transform) {
  let result = '';
  let previous = 0;
  for (const match of markup.matchAll(pattern)) {
    const index = match.index ?? 0;
    const assetPath = match[1];
    result += markup.slice(previous, index);
    result += await transform(assetPath);
    previous = index + match[0].length;
  }
  return result + markup.slice(previous);
}

function hashValue(content) {
  return `sha256-${createHash('sha256').update(content).digest('base64')}`;
}

function quoteHash(value) {
  return `'${value}'`;
}

function scriptContents() {
  return [...html.matchAll(/<script type="module">([\s\S]*?)<\/script>/g)].map((match) => match[1]);
}

function styleContents() {
  return [...html.matchAll(/<style>([\s\S]*?)<\/style>/g)].map((match) => match[1]);
}
