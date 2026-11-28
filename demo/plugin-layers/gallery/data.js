window.D = {
 "nodes": [
  {
   "name": "AboutME_ModuleRelationshipNetwork",
   "pkg": "aboutme-module-relationship-network",
   "vendor": "AboutME",
   "version": "0.1.0",
   "layer": "app",
   "desc": "模块关系网插件",
   "requires": [
    "nous-routes"
   ],
   "endpoints": 1,
   "caps": false,
   "unlisted": true,
   "note": "",
   "deps": [
    "routes"
   ],
   "kind": "unlisted",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "archive",
   "pkg": "data-archive",
   "vendor": "Data",
   "version": "0.2.1",
   "layer": "app",
   "desc": "应用层 archive 模块",
   "requires": [
    "nous-durable",
    "nous-routes"
   ],
   "endpoints": 3,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "durable",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "data-manager",
   "pkg": "data-data-manager",
   "vendor": "Data",
   "version": "0.4.0",
   "layer": "app",
   "desc": "应用层 data-manager 模块",
   "requires": [
    "nous-chord",
    "nous-durable",
    "nous-routes",
    "nous-store"
   ],
   "endpoints": 8,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "chord",
    "durable",
    "routes",
    "store"
   ],
   "kind": "module",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "Models",
   "pkg": "element-models",
   "vendor": "Element",
   "version": "0.3.0",
   "layer": "app",
   "desc": "models 插件",
   "requires": [
    "nous-elements",
    "nous-env",
    "nous-repo",
    "nous-routes",
    "nous-runtime"
   ],
   "endpoints": 2,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "elements",
    "env",
    "repo",
    "routes",
    "runtime"
   ],
   "kind": "module",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "Extend",
   "pkg": "library-extend",
   "vendor": "Library",
   "version": "0.10.0",
   "layer": "app",
   "desc": "扩展监管",
   "requires": [
    "nous-durable",
    "nous-ext-sdk",
    "nous-repo",
    "nous-routes"
   ],
   "endpoints": 15,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "durable",
    "ext-sdk",
    "repo",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "ext-probe",
    "server"
   ]
  },
  {
   "name": "Skill",
   "pkg": "library-skill",
   "vendor": "Library",
   "version": "0.4.0",
   "layer": "app",
   "desc": "Skill 插件",
   "requires": [
    "nous-env",
    "nous-repo",
    "nous-routes"
   ],
   "endpoints": 6,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "env",
    "repo",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "ai",
   "pkg": "nous-ai",
   "vendor": "Nous",
   "version": "0.2.2",
   "layer": "platform",
   "desc": "Unified multi-provider LLM types + streaming. 冻结自 pi-rust@v0.24.2 的 rpi-ai",
   "requires": [
    "nous-ai-types",
    "nous-telemetry"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "ai-types",
    "telemetry"
   ],
   "kind": "lib",
   "dependents": [
    "Trigger_Core",
    "runtime",
    "server",
    "sessions",
    "tools"
   ]
  },
  {
   "name": "ai-types",
   "pkg": "nous-ai-types",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "kernel",
   "desc": "nous-ai 的纯半",
   "requires": [],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [],
   "kind": "lib",
   "dependents": [
    "ai",
    "durable"
   ]
  },
  {
   "name": "chord",
   "pkg": "nous-chord",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "kernel",
   "desc": "chord 的 context / json / delta / sqlite 语义层",
   "requires": [],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [],
   "kind": "lib",
   "dependents": [
    "data-manager",
    "dispatcher",
    "durable",
    "env"
   ]
  },
  {
   "name": "combos",
   "pkg": "nous-combos",
   "vendor": "Nous",
   "version": "0.2.7",
   "layer": "app",
   "desc": "combos 插件",
   "requires": [
    "nous-dispatcher",
    "nous-durable",
    "nous-repo",
    "nous-routes"
   ],
   "endpoints": 4,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "dispatcher",
    "durable",
    "repo",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "Device",
   "pkg": "nous-device",
   "vendor": "Nous",
   "version": "0.11.0",
   "layer": "app",
   "desc": "device 插件",
   "requires": [
    "nous-routes"
   ],
   "endpoints": 9,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "presets-admin",
    "server",
    "sessions"
   ]
  },
  {
   "name": "dispatcher",
   "pkg": "nous-dispatcher",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "app",
   "desc": "#73",
   "requires": [
    "nous-chord",
    "nous-durable",
    "nous-store"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "chord",
    "durable",
    "store"
   ],
   "kind": "lib",
   "dependents": [
    "Trigger_Core",
    "combos",
    "server"
   ]
  },
  {
   "name": "durable",
   "pkg": "nous-durable",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "kernel",
   "desc": "durable 的语义自己写",
   "requires": [
    "nous-ai-types",
    "nous-chord",
    "nous-wire"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "ai-types",
    "chord",
    "wire"
   ],
   "kind": "lib",
   "dependents": [
    "Extend",
    "Trigger_Core",
    "archive",
    "combos",
    "data-manager",
    "dispatcher",
    "ext-probe",
    "fixture",
    "server",
    "sessions"
   ]
  },
  {
   "name": "elements",
   "pkg": "nous-elements",
   "vendor": "Nous",
   "version": "0.3.1",
   "layer": "app",
   "desc": "Nous 元素库（组 / 包）",
   "requires": [
    "nous-env",
    "nous-repo",
    "nous-routes"
   ],
   "endpoints": 8,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "env",
    "repo",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "Models",
    "server"
   ]
  },
  {
   "name": "env",
   "pkg": "nous-env",
   "vendor": "Nous",
   "version": "0.3.3",
   "layer": "platform",
   "desc": "ExecutionEnv 契约",
   "requires": [
    "nous-chord",
    "nous-routes"
   ],
   "endpoints": 1,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "chord",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "Models",
    "Nous_Prompts",
    "Skill",
    "elements",
    "env-ssh",
    "presets-admin",
    "repo",
    "server",
    "tools"
   ]
  },
  {
   "name": "env-ssh",
   "pkg": "nous-env-ssh",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "platform",
   "desc": "ExecutionEnv 的 SSH 远端实现",
   "requires": [
    "nous-env"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "env"
   ],
   "kind": "lib",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "ext-probe",
   "pkg": "nous-ext-probe",
   "vendor": "Nous",
   "version": "0.2.2",
   "layer": "dev",
   "desc": "判据执行器",
   "requires": [
    "library-extend",
    "nous-durable",
    "nous-ext-sdk",
    "nous-server",
    "nous-wire"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "Extend",
    "durable",
    "ext-sdk",
    "server",
    "wire"
   ],
   "kind": "lib",
   "dependents": []
  },
  {
   "name": "ext-sdk",
   "pkg": "nous-ext-sdk",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "platform",
   "desc": "扩展作者面",
   "requires": [],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [],
   "kind": "lib",
   "dependents": [
    "Extend",
    "ext-probe"
   ]
  },
  {
   "name": "fixture",
   "pkg": "nous-fixture",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "dev",
   "desc": "读 fixtures/agent.sqlite，往 stdout 打 /v1/sessions 形状的 JSON（verify.sh 第 2 段的对象）",
   "requires": [
    "nous-durable"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "durable"
   ],
   "kind": "lib",
   "dependents": []
  },
  {
   "name": "presets-admin",
   "pkg": "nous-presets-admin",
   "vendor": "Nous",
   "version": "0.3.4",
   "layer": "app",
   "desc": "模板管理（Nous_PresetsAdmin）",
   "requires": [
    "nous-device",
    "nous-env",
    "nous-prompts",
    "nous-repo",
    "nous-routes",
    "nous-store"
   ],
   "endpoints": 19,
   "caps": true,
   "unlisted": false,
   "note": "同 crate 的 presets 另有 13 条端点（caps 里是两个模块名，graph 里一个节点）",
   "deps": [
    "Device",
    "env",
    "Nous_Prompts",
    "repo",
    "routes",
    "store"
   ],
   "kind": "module",
   "dependents": [
    "server"
   ]
  },
  {
   "name": "Nous_Prompts",
   "pkg": "nous-prompts",
   "vendor": "Nous",
   "version": "0.4.0",
   "layer": "app",
   "desc": "提示词目录只读扫描与纯 render()（#72）",
   "requires": [
    "nous-env",
    "nous-repo",
    "nous-routes"
   ],
   "endpoints": 6,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "env",
    "repo",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "presets-admin",
    "server"
   ]
  },
  {
   "name": "repo",
   "pkg": "nous-repo",
   "vendor": "Nous",
   "version": "0.7.0",
   "layer": "platform",
   "desc": "#75",
   "requires": [
    "nous-env",
    "nous-routes"
   ],
   "endpoints": 30,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "env",
    "routes"
   ],
   "kind": "module",
   "dependents": [
    "Extend",
    "Models",
    "Nous_Prompts",
    "Skill",
    "Trigger_Core",
    "combos",
    "elements",
    "presets-admin",
    "server"
   ]
  },
  {
   "name": "routes",
   "pkg": "nous-routes",
   "vendor": "Nous",
   "version": "0.3.0",
   "layer": "platform",
   "desc": "插件自带端点的契约",
   "requires": [],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [],
   "kind": "lib",
   "dependents": [
    "AboutME_ModuleRelationshipNetwork",
    "Device",
    "Extend",
    "Models",
    "Nous_Prompts",
    "Skill",
    "Trigger_Core",
    "archive",
    "combos",
    "data-manager",
    "elements",
    "env",
    "presets-admin",
    "repo",
    "server",
    "sessions"
   ]
  },
  {
   "name": "runtime",
   "pkg": "nous-runtime",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "platform",
   "desc": "Embeddable model catalog, credential resolution and provider execution bindings",
   "requires": [
    "nous-ai"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "ai"
   ],
   "kind": "lib",
   "dependents": [
    "Models",
    "server"
   ]
  },
  {
   "name": "server",
   "pkg": "nous-server",
   "vendor": "Nous",
   "version": "0.4.0",
   "layer": "app",
   "desc": "wire.md 的端点层",
   "requires": [
    "aboutme-module-relationship-network",
    "data-archive",
    "data-data-manager",
    "element-models",
    "library-extend",
    "library-skill",
    "nous-ai",
    "nous-combos",
    "nous-device",
    "nous-dispatcher",
    "nous-durable",
    "nous-elements",
    "nous-env",
    "nous-env-ssh",
    "nous-presets-admin",
    "nous-prompts",
    "nous-repo",
    "nous-routes",
    "nous-runtime",
    "nous-sessions",
    "nous-store",
    "nous-telemetry",
    "nous-tools",
    "nous-wire",
    "trigger-core"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "AboutME_ModuleRelationshipNetwork",
    "archive",
    "data-manager",
    "Models",
    "Extend",
    "Skill",
    "ai",
    "combos",
    "Device",
    "dispatcher",
    "durable",
    "elements",
    "env",
    "env-ssh",
    "presets-admin",
    "Nous_Prompts",
    "repo",
    "routes",
    "runtime",
    "sessions",
    "store",
    "telemetry",
    "tools",
    "wire",
    "Trigger_Core"
   ],
   "kind": "lib",
   "dependents": [
    "ext-probe"
   ]
  },
  {
   "name": "sessions",
   "pkg": "nous-sessions",
   "vendor": "Nous",
   "version": "0.8.0",
   "layer": "app",
   "desc": "最小轮次（pi 本体）+ 会话的读投影 / 写命令 / /v1/sessions* 端点",
   "requires": [
    "nous-ai",
    "nous-device",
    "nous-durable",
    "nous-routes",
    "nous-wire"
   ],
   "endpoints": 17,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "ai",
    "Device",
    "durable",
    "routes",
    "wire"
   ],
   "kind": "module",
   "dependents": [
    "Trigger_Core",
    "server"
   ]
  },
  {
   "name": "store",
   "pkg": "nous-store",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "platform",
   "desc": "Nous 自己的库（nous.db）",
   "requires": [],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [],
   "kind": "lib",
   "dependents": [
    "Trigger_Core",
    "data-manager",
    "dispatcher",
    "presets-admin",
    "server"
   ]
  },
  {
   "name": "telemetry",
   "pkg": "nous-telemetry",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "platform",
   "desc": "Telemetry span/event contracts (noop default). 冻结自 pi-rust@v0.24.2 的 rpi-telemetry",
   "requires": [],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [],
   "kind": "lib",
   "dependents": [
    "ai",
    "server"
   ]
  },
  {
   "name": "tools",
   "pkg": "nous-tools",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "platform",
   "desc": "Built-in agent tools (read/write/edit/bash/grep/find/ls) — frozen port of rpi-tools@v0.24.2, with tools depending only on nous-ai + nous-env",
   "requires": [
    "nous-ai",
    "nous-env"
   ],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [
    "ai",
    "env"
   ],
   "kind": "lib",
   "dependents": [
    "Trigger_Core",
    "server"
   ]
  },
  {
   "name": "wire",
   "pkg": "nous-wire",
   "vendor": "Nous",
   "version": "0.2.0",
   "layer": "kernel",
   "desc": "wire.md 的 Rust 投影",
   "requires": [],
   "endpoints": 0,
   "caps": false,
   "unlisted": false,
   "note": "",
   "deps": [],
   "kind": "lib",
   "dependents": [
    "durable",
    "ext-probe",
    "server",
    "sessions"
   ]
  },
  {
   "name": "Trigger_Core",
   "pkg": "trigger-core",
   "vendor": "Trigger",
   "version": "0.2.0",
   "layer": "app",
   "desc": "触发器",
   "requires": [
    "nous-ai",
    "nous-dispatcher",
    "nous-durable",
    "nous-repo",
    "nous-routes",
    "nous-sessions",
    "nous-store",
    "nous-tools"
   ],
   "endpoints": 24,
   "caps": true,
   "unlisted": false,
   "note": "",
   "deps": [
    "ai",
    "dispatcher",
    "durable",
    "repo",
    "routes",
    "sessions",
    "store",
    "tools"
   ],
   "kind": "module",
   "dependents": [
    "server"
   ]
  }
 ],
 "counts": {
  "all": 31,
  "modules": 14,
  "libs": 16,
  "unlisted": 1,
  "endpoints": 165,
  "capsModules": 15,
  "edges": 96
 },
 "missing": []
};
