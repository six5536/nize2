#!/usr/bin/env node

import { Command } from "commander";
import { version } from "./index.js";

const program = new Command();

program.name("nize").description("Nize MCP — Model Context Protocol tools").version(version());

program.parse();
