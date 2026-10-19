#!/usr/bin/env node
// Versão do app nos quatro arquivos que a carregam. Uso:
//
//   pnpm release:version 1.2.0           grava a versão (passo 1 do lançamento)
//   node scripts/release-version.mjs check app-v1.2.0
//                                        confere a tag contra os arquivos (CI)
//
// O lançamento é por tag — ver "Lançar uma versão" no README.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const TAG_PREFIX = 'app-v';
// semver com pre-release opcional (1.2.0, 1.2.0-beta.3), sem build metadata
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/;

// Cada arquivo: como achar a linha da versão do app. Edição por regex, para
// não reformatar o resto do arquivo.
const FILES = [
  { path: 'package.json', pattern: /^( {2}"version": ")([^"]+)(")/m },
  {
    path: 'src-tauri/tauri.conf.json',
    pattern: /^( {2}"version": ")([^"]+)(")/m,
  },
  {
    path: 'src-tauri/Cargo.toml',
    pattern: /^(\[package\]\nname = "octapus_db"\nversion = ")([^"]+)(")/m,
  },
  {
    path: 'src-tauri/Cargo.lock',
    pattern: /^(\[\[package\]\]\nname = "octapus_db"\nversion = ")([^"]+)(")/m,
  },
];

function read(file) {
  const text = readFileSync(root + file.path, 'utf8');
  const match = text.match(file.pattern);
  if (!match) throw new Error(`versão não encontrada em ${file.path}`);
  return { text, version: match[2] };
}

function fail(message) {
  console.error(`erro: ${message}`);
  process.exit(1);
}

function setVersion(version) {
  if (!SEMVER.test(version ?? '')) {
    fail(
      `"${version ?? ''}" não é uma versão semver (ex.: 1.2.0 ou 1.2.0-beta.1)`,
    );
  }
  for (const file of FILES) {
    const { text, version: previous } = read(file);
    writeFileSync(
      root + file.path,
      text.replace(file.pattern, `$1${version}$3`),
    );
    console.log(`${file.path}: ${previous} -> ${version}`);
  }
  const tag = TAG_PREFIX + version;
  console.log(`
Próximos passos:
  git commit -am "${version}"
  git tag -a ${tag} -m "octapus-db ${version}"
  git push origin HEAD ${tag}`);
}

function check(tag) {
  if (
    !tag?.startsWith(TAG_PREFIX) ||
    !SEMVER.test(tag.slice(TAG_PREFIX.length))
  ) {
    fail(
      `a tag "${tag ?? ''}" não é ${TAG_PREFIX}<versão> (ex.: ${TAG_PREFIX}1.2.0)`,
    );
  }
  const expected = tag.slice(TAG_PREFIX.length);
  const wrong = FILES.map(file => ({
    path: file.path,
    version: read(file).version,
  })).filter(file => file.version !== expected);
  if (wrong.length > 0) {
    for (const file of wrong) console.error(`${file.path}: ${file.version}`);
    fail(
      `a tag ${tag} não bate com a versão dos arquivos acima — rode "pnpm release:version ${expected}" e refaça a tag`,
    );
  }
  console.log(
    `${tag}: versão ${expected} confere nos ${FILES.length} arquivos`,
  );
}

const [command, value] = process.argv.slice(2);
if (command === 'check') check(value);
else setVersion(command);
