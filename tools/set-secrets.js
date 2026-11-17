#!/usr/bin/env node
/**
 * 用仓库的 GitHub Actions secrets API 写入固定签名材料。
 *
 * 为什么这么做：keystore 不能进仓库（公开仓库等于泄私钥），
 * 但签名必须"固化"才能覆盖安装。GitHub 的 secrets 用 libsodium sealed box
 * 加密传输，这里用仓库公钥加密后 PUT 上去。
 *
 * 用法：node set-secrets.js <owner/repo> <token> <keystorePath>
 */
const fs = require('fs');
const path = require('path');

const [, , repo, token, keystorePath] = process.argv;
if (!repo || !token || !keystorePath) {
  console.error('usage: node set-secrets.js <owner/repo> <token> <keystore-path>');
  process.exit(1);
}

const KEYSTORE = path.resolve(keystorePath);
const PASSWORD = process.env.ANDROID_KEYSTORE_PASSWORD || 'BaYinCar#2026';
const ALIAS = process.env.ANDROID_KEY_ALIAS || 'bayin';
const KEY_PASSWORD = process.env.ANDROID_KEY_PASSWORD || PASSWORD;

const API = 'https://api.github.com';
const headers = {
  Authorization: `token ${token}`,
  Accept: 'application/vnd.github+json',
  'User-Agent': 'bayin-car-ci',
  'Content-Type': 'application/json'
};

async function getPublicKey() {
  const r = await fetch(`${API}/repos/${repo}/actions/secrets/public-key`, { headers });
  if (!r.ok) throw new Error(`public-key ${r.status} ${await r.text()}`);
  return r.json();
}

async function putSecret(publicKey, name, value) {
  const sodium = require('libsodium-wrappers');
  await sodium.ready;
  const binKey = sodium.from_base64(publicKey.key, sodium.base64_variants.ORIGINAL);
  const encBytes = sodium.crypto_box_seal(sodium.from_string(value), binKey);
  const encrypted = sodium.to_base64(encBytes, sodium.base64_variants.ORIGINAL);

  const r = await fetch(`${API}/repos/${repo}/actions/secrets/${name}`, {
    method: 'PUT',
    headers,
    body: JSON.stringify({ encrypted_value: encrypted, key_id: publicKey.key_id })
  });
  if (!r.ok && r.status !== 201 && r.status !== 204) {
    throw new Error(`put ${name} ${r.status} ${await r.text()}`);
  }
}

(async () => {
  const key = await getPublicKey();
  const keystoreB64 = fs.readFileSync(KEYSTORE).toString('base64');

  const secrets = {
    ANDROID_KEYSTORE_BASE64: keystoreB64,
    ANDROID_KEYSTORE_PASSWORD: PASSWORD,
    ANDROID_KEY_ALIAS: ALIAS,
    ANDROID_KEY_PASSWORD: KEY_PASSWORD
  };

  for (const [name, value] of Object.entries(secrets)) {
    await putSecret(key, name, value);
    console.log('secret set:', name, `(${value.length} chars)`);
  }
  console.log('done');
})().catch(e => {
  console.error('FAILED:', e.message);
  process.exit(1);
});