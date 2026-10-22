// Downloads the sentence-embedding model the classifier uses for
// title-vs-goal similarity (all-MiniLM-L6-v2, Apache-2.0) into data/models/.
// Run: node scripts/fetch-model.mjs
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const REPO = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve";
// Pinned to a commit so the files (and their hashes) never change under us.
const REVISION = "c9745ed1d9f207416be6d2e6f8de32d1f16199bf";
const FILES = [
  { path: "onnx/model.onnx", name: "model.onnx", sha256: "6fd5d72fe4589f189f8ebc006442dbb529bb7ce38f8082112682524616046452" },
  { path: "vocab.txt", name: "vocab.txt", sha256: "07eced375cec144d27c900241f3e339478dec958f92fddbc551f295c992038a3" },
];
const OUT = join(import.meta.dirname, "..", "data", "models", "all-MiniLM-L6-v2");

mkdirSync(OUT, { recursive: true });
for (const file of FILES) {
  const target = join(OUT, file.name);
  const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
  if (existsSync(target) && (!file.sha256 || sha(readFileSync(target)) === file.sha256)) {
    console.log(`${file.name}: already here`);
    continue;
  }
  const res = await fetch(`${REPO}/${REVISION}/${file.path}`);
  if (!res.ok) throw new Error(`${file.name}: HTTP ${res.status}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  const hash = sha(bytes);
  if (file.sha256 && hash !== file.sha256) throw new Error(`${file.name}: sha256 ${hash}, expected ${file.sha256}`);
  writeFileSync(target, bytes);
  console.log(`${file.name}: ${(bytes.length / 1e6).toFixed(1)} MB, sha256 ${hash}`);
}
