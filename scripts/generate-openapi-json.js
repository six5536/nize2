#!/usr/bin/env node
// Converts TypeSpec-generated OpenAPI YAML to JSON for swagger docs / client codegen.

import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import yaml from "js-yaml";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = join(__dirname, "..");

const OPENAPI_YAML = join(ROOT_DIR, "codegen", "nize-api", "tsp-output", "@typespec", "openapi3", "openapi.yaml");
const OPENAPI_JSON = join(ROOT_DIR, "codegen", "nize-api", "tsp-output", "openapi.json");

function log(msg) {
  console.log(`[generate-openapi-json] ${msg}`);
}

log(`Reading ${OPENAPI_YAML}`);
const content = readFileSync(OPENAPI_YAML, "utf8");
const doc = yaml.load(content);

log(`Writing ${OPENAPI_JSON}`);
writeFileSync(OPENAPI_JSON, JSON.stringify(doc, null, 2));
log("Done.");
