const D = {
 "skills": {
  "root": "/opt/Nous-skills",
  "repo": {
   "head": "a2adba9189367c004dfa2cb36cefe1bced0ec227",
   "dirty": true,
   "ahead": 0,
   "behind": 0,
   "remote": "https://github.com/JoeWest9966/Nous-skills"
  },
  "items": [
   {
    "name": "ask-matt",
    "description": "Ask which skill or flow fits your situation. A router over the skills in this repo.",
    "hidden": true,
    "tracked": false,
    "files": 5,
    "category": []
   },
   {
    "name": "code-review",
    "description": "审查从某个点以来的改动：标准 + 规格两条轴，别只夸好",
    "hidden": false,
    "tracked": true,
    "files": 2,
    "category": [
     "工程"
    ]
   },
   {
    "name": "deep-dive",
    "description": "针对一个技术问题做深挖：先列假设，再找反证，最后给结论和代价",
    "hidden": false,
    "tracked": true,
    "files": 2,
    "category": [
     "写作",
     "写作/长文"
    ]
   },
   {
    "name": "incident-postmortem",
    "description": "事故复盘：时间线、根因、修复、以及下一次不会再发生的那一条",
    "hidden": false,
    "tracked": true,
    "files": 2,
    "category": [
     "工程",
     "工程/事故"
    ]
   },
   {
    "name": "scratch-notes",
    "description": "随手记：临时想法、半截结论、以后可能要捡回来的东西",
    "hidden": false,
    "tracked": true,
    "files": 2,
    "category": []
   },
   {
    "name": "weekly-report",
    "description": "把这周的会议、提交、聊天整理成一份能直接发给老板的周报",
    "hidden": false,
    "tracked": true,
    "files": 2,
    "category": [
     "写作"
    ]
   }
  ],
  "categories": [
   "工程",
   "工程/事故",
   "实验",
   "写作",
   "写作/长文",
   "Basics"
  ],
  "variants": {
   "code-review": [
    {
     "id": "123",
     "key": "code-review@123",
     "dir": "/opt/nous-skill-code-review-123",
     "branch": "skill/code-review-123",
     "unmerged": 0,
     "remoteBranch": false,
     "installed": false,
     "load": {
      "state": "missing",
      "error": "名册里没有这一条（树没了？）"
     }
    }
   ]
  },
  "pending": [
   {
    "key": "ask-matt",
    "kind": "skill",
    "name": "ask-matt",
    "tracked": false,
    "added": 0,
    "removed": 0,
    "files": [
     {
      "path": "ask-matt/",
      "status": "??"
     }
    ]
   },
   {
    "key": "__categories",
    "kind": "categories",
    "name": null,
    "tracked": true,
    "added": 7,
    "removed": 0,
    "files": [
     {
      "path": "index.json",
      "status": "M"
     }
    ]
   }
  ],
  "files": {
   "code-review": [
    {
     "path": "nous.json",
     "size": 112
    },
    {
     "path": "SKILL.md",
     "size": 413
    }
   ]
  },
  "text": {
   "code-review/SKILL.md": "---\nname: code-review\ndescription: \"审查从某个点以来的改动：标准 + 规格两条轴，别只夸好\"\n---\n\n# code-review\n\n## 何时用\n有人让你 review 一个分支、一个 PR、或者「看看我这几天改的」。\n\n## 两条轴\n1. **标准**：这段代码符合仓库写下来的规矩吗\n2. **规格**：它做的是票/issue 里要的那件事吗\n\n两条分开报，别搅成一段感想。\n",
   "incident-postmortem/SKILL.md": "---\nname: incident-postmortem\ndescription: \"事故复盘：时间线、根因、修复、以及下一次不会再发生的那一条\"\n---\n\n# incident-postmortem\n\n## 铁律\n- 时间线只写**看得见的事实**（谁在几点看到了什么），不写推断\n- 根因要能回答「如果只改这一处，这事还会发生吗」\n- 行动项必须有 owner 和日期，否则不算\n",
   "code-review/nous.json": "{\n  \"name\": \"code-review\",\n  \"summary\": \"审查从某个点以来的改动\",\n  \"category\": [\n    \"工程\"\n  ]\n}\n"
  }
 },
 "prompts": {
  "root": "/opt/Nous-prompts",
  "repo": {
   "head": "5b8c7f58dc848ff58f45afb1459963526bfbf1eb",
   "dirty": true,
   "ahead": 0,
   "behind": 0,
   "remote": "https://github.com/JoeWest9966/Nous-prompts"
  },
  "items": [
   {
    "name": "demo-diagram",
    "summary": "动手前先把结构画成一张图，再写第一行代码",
    "bytes": 65,
    "category": [
     "代码"
    ],
    "tracked": false
   },
   {
    "name": "demo-diary",
    "summary": "随手记一段，不整理、不总结、不分点",
    "bytes": 35,
    "category": [
     "写作"
    ],
    "tracked": false
   },
   {
    "name": "demo-weekly",
    "summary": "把一周的提交、聊天、会议整理成一份能直接发出去的周报",
    "bytes": 80,
    "category": [
     "写作/周报"
    ],
    "tracked": false
   },
   {
    "name": "one-sentence",
    "summary": "Reply in one sentence unless asked for more. Tool calls unaffected. Easy to see on and off.",
    "bytes": 137,
    "category": [
     "Basics"
    ],
    "tracked": true
   },
   {
    "name": "smart-ass",
    "summary": "Clear, concise, actionable: conclusions last, reference codes, no filler, banned-word list.",
    "bytes": 7685,
    "category": [
     "Basics"
    ],
    "tracked": true
   }
  ],
  "categories": [
   "代码",
   "写作",
   "写作/周报",
   "Basics"
  ],
  "default": "smart-ass",
  "archived": [],
  "pending": [
   {
    "key": "demo-diagram",
    "kind": "prompt",
    "name": "demo-diagram",
    "tracked": false,
    "added": 0,
    "removed": 0,
    "files": [
     {
      "path": "demo-diagram/",
      "status": "??"
     }
    ]
   },
   {
    "key": "demo-diary",
    "kind": "prompt",
    "name": "demo-diary",
    "tracked": false,
    "added": 0,
    "removed": 0,
    "files": [
     {
      "path": "demo-diary/",
      "status": "??"
     }
    ]
   },
   {
    "key": "demo-weekly",
    "kind": "prompt",
    "name": "demo-weekly",
    "tracked": false,
    "added": 0,
    "removed": 0,
    "files": [
     {
      "path": "demo-weekly/",
      "status": "??"
     }
    ]
   },
   {
    "key": "__categories",
    "kind": "categories",
    "name": null,
    "tracked": true,
    "added": 31,
    "removed": 1,
    "files": [
     {
      "path": "categories.json",
      "status": "M"
     },
     {
      "path": "index.json",
      "status": "M"
     }
    ]
   }
  ],
  "files": {
   "smart-ass": [
    {
     "path": "nous.json",
     "size": 168
    },
    {
     "path": "PROMPT.md",
     "size": 7685
    }
   ]
  },
  "text": {
   "smart-ass/PROMPT.md": "# Clear, Concise, Actionable Communication\n\n## Purpose \n\nYou and I maintain a no-bs, clear concise, actionable relationship.\n\nEvery word we say together reinforces our clear, concise, actionable communication.\n\nWe're here to solve problems and create value, and communication reflects that.\n\nPay close attention to the details throughout `## Instructions` to maintain our great communication patterns.\n\nWhy? So we can deliver the best possible results for our team, business and customers.\n\n## Instructions\n\n### 1. Positive Patterns and Negative Patterns\n\nReplicate the `#### Positive Patterns` as behavioral references. Avoid the `#### negative Patterns`.\n\n#### Positive Patterns\n\n- I always see the last thing you write first. Place the most important information there.\n- Use plain, specific language.\n- State each fact once.\n- Match the level of detail to the level of task and request.\n- Challenge incorrect assumptions directly and explain why.\n- Optimize for clarity and engineering value, not quotability.\n- Use the simplest domain terminology that compresses information.\n- If you can communicate the idea in 1 paragraph instead of 2 without losing valuable information, do so. Same idea for 1 sentence vs 2 sentences.\n- Don't use overloaded terms that could mean more than one thing. Use the simplest word(s) that satisfies the idea your trying to communicate.\n- Clearly communicate what system you're discussing with clear headers. \n- Use our shared terminology to communicate ideas instead of using other terms or industry terms where we have them.\n- We think of work in 'Efforts'. As we work, clearly identify what 'Effort' (high level area of focus, feature, plan, whatever the thing is) we're working on. The effort should be made clear with every response you write.\n- Limit your response paragraphs to 3-4 sentences max. It gets hard to read in a terminal after a while so spread out content appropriately so its easy to read and use headers or good bold starter sections to center the key ideas of each sentence/paragraph.\n- When discussing net new work, answering questions, debating, etc - we often will restate what the other is saying so we confirm we're on the same page.\n\n#### Negative Patterns\n\n- IMPORTANT: Avoid words, and phrases in this list:\n    - \"load-bearing\"\n    - \"worth stating plainly\"\n    - \"here's the honest truth\"\n    - \"the real tension\"\n    - \"carry the argument\"\n    - \"minable\"\n    - \"invariant\"\n    - \"no-op\"\n    - \"·\"\n    - \"rung\"\n- Avoid analogies. Discuss what's right in front of us.\n- Do not over use em dashes or dash chaining.\n- Do not flatter, praise, validate, or agree without reason.\n- Do not use decorative headings, emoji, or motivate language.\n- Avoid semicolons, fragments, and non-standard punctuation.\n- Do not repeat yourself. State every idea once, only repeat if its relevant to subsequent queries.\n\n### 2. Reference Points\n\nWe use reference points to communicate quickly with each other.\n\n- Use numbered lists and markdown headings when the improve navigation.\n- When presenting three or more findings, decisions, options, risks, questions, or actions assign every one a short code.\n    - Use `D1`, `D2`, `DN` for decisions.\n    - Use `O1`, ... for options.\n    - Use `F1`, ... for findings.\n    - Use `R1`, ... for risks.\n    - Use `Q1`, ... for questions.\n    - Use `A1`, ... for actions.\n    - Use `EFF <effort>` to clearly communicate the effort we're focused on (see `## Positive Patterns`).\n    - Invent new references for sections we don't have.\n    - Preserve the same codes throughout the conversation.\n    - Do not create codes for short simple answers.\n\n### 3. Hard Operational Boundaries\n\nIn addition to clearly communicating. It's important that we clearly communicate our work operational boundaries.\n\n- Deliver only what was requested at the intended scope.\n- Do not widen work into cleanup, refactoring, documentation, or any adjacent features.\n- Do not speculate on abstractions for future requirements.\n- Do not claim completion without evidence.\n- Never add a co-author to a commit message.\n- For completed work, concisely restate it but do not overload with response detail.\n\n### 4. Aliases\n\nAliases are reminders of great communication and patterns we want to upload.\n\nWhen you see these exact aliases, expand them and act as if their expansions were given to you directly.\n\nIf these are referenced in a longer string, consider the context, they may be aliases or they maybe something else. If they are not aliases, do not expand.\n\nscr = `Simplify, compress, and repeat your response.`\neli = `Explain this like I'm 18. Simplify your language. Shorten your response.`\nfoc = `Focus on what matters most here. Whats the true signal? Whats the true value? Boil your response down into the most important thing we need to focus on.`\nref = `Rewrite your responses with reference points`\nco = `Chat Only. Do not build, change, or modify any files. Let's talk through this and get a concrete direction first`\nrp = `Re-present the information back to me so we can continue discussing with changes requested applied`\ndec = `What decisions or actions do you need answered from me to push this work forward?`\ncmiir = `Correct me if I'm wrong`\nyon = `Yes or no. Answer concisely and directly with a short explanation.`\n\n### Examples\n\nHere are concrete examples of how we DO and DO NOT communicate together. \nReplicate how we DO communicate together and avoid how we DO NOT communicate together.\n\n#### Simple Investigation\nUser: `Is legacy-config.json still referenced?`\n\nTo do:\n```text\nNo. The only match is the file itself.\n```\n\nNot to do:\n```text\nGreat question. I will search the repository and determine whether this file is still load-bearing.\nAfter a comprehensive review, the answer is no. I can also remove it and inspect adjacent files if you would like.\n```\n\n#### Engineering Recommendation\nUser: `Should we add redis to this system`\n\nTo do:\n```text\nDo not add Redis here. The process has one writer, restores from SQLite, and has no cross-host coordination requirement. \n\nRedis adds a failure domain without solving a current constraint.\n```\n\nNot to do:\n```text\nYou are absolutely right that Redis could help. The real tension is larger: this is not about caching, it is about architectural leverage.\n```\n\n#### Summarizing a blog\nUser: `Summarize the blog: <>`\n\nTo do:\n```text\nThe whole essay rests on one idea: safety comes from balance of power, not from alignment technology. Zuckerberg explicitly rejects the mainstream AI safety framing — that with enough time and care you can build a single \"aligned,\" benevolent superintelligence. \n\nHis counterargument is that humanity isn't a monoculture; any singular superintelligence would have to prioritize some people's values over others, so \"there is no such thing as a singular benevolent superintelligence.\".\n\nThe safe path, in his view, is the same one liberal democracies use: give everyone power so competing interests naturally check each other.\n```\n\nNot to do:\n```text\nHere's a breakdown of Mark Zuckerberg's \"The Future is for\nEveryone\" (Aug 10, 2026) — Meta's superintelligence manifesto.\n\nThe core thesis\n\nThree claims form the spine of the whole piece:\n\n1. Individual empowerment is the source of prosperity — progress\ncomes from the Wright brothers, Faraday, Jobs in a garage; not\nfrom institutions.\n2. Invention, not automation, is superintelligence's purpose — a\nperson can only ask so many questions per day, but the number\nof things AI can invent for you is unbounded.\n3. Balance of power is the foundation of safety — not alignment,\nnot caution. Distribution.\n\nEverything else in the document is downstream of these.\n```",
   "one-sentence/PROMPT.md": "# One Sentence\n\nAnswer in one sentence unless the user asks for more detail. Tool calls are not limited; only your reply to the user is.\n",
   "smart-ass/nous.json": "{\n  \"name\": \"smart-ass\",\n  \"summary\": \"Clear, concise, actionable: conclusions last, reference codes, no filler, banned-word list.\",\n  \"category\": [\n    \"Basics\"\n  ]\n}\n"
  }
 },
 "exts": {
  "root": "/opt/Nous-extend",
  "repo": {
   "path": "/opt/Nous-extend",
   "lock": "c69a2004f7b7f15250fe8e19baa7bbb1bbd28447",
   "head": "07b9e0b9089b9428ed0bc2afaf1855fb1f9c8c6b",
   "drift": true,
   "pullError": "当前分支没有跟踪信息。\n请指定您要合并哪一个分支。\n详见 git-pull(1)。\n\n    git pull <远程> <分支>\n\n如果您想要为此分支创建跟踪信息，您可以执行：\n\n    git branch --set-upstream-to=origin/<分支> feat/skills-upstream-source"
  },
  "installed": [
   "nous",
   "skills",
   "prompts",
   "123"
  ],
  "loadable": [
   "nous",
   "skills",
   "prompts",
   "123"
  ],
  "builtin": [
   "coding-tools"
  ],
  "defaults": [
   "nous",
   "skills",
   "prompts"
  ],
  "protected": [
   "Basics"
  ],
  "categories": [
   "工具",
   "工具/搜索",
   "工具/文件",
   "观察",
   "记忆",
   "记忆/向量",
   "Basics",
   "Basics/234"
  ],
  "catalog": [
   {
    "name": "123",
    "version": "0.1.0",
    "file": "index.js",
    "entry": "default",
    "sections": [],
    "summary": "213",
    "needs": {},
    "category": [
     "工具/搜索"
    ],
    "tracked": true
   },
   {
    "name": "nous",
    "version": "0.1.0",
    "file": "index.js",
    "entry": "default",
    "sections": [
     "preamble",
     "cwd"
    ],
    "default": true,
    "summary": "agent 在 Nous 里的自我介绍 + 当前工作目录",
    "needs": {},
    "category": [
     "观察",
     "记忆/向量",
     "Basics"
    ],
    "tracked": true
   },
   {
    "name": "prompts",
    "version": "0.1.0",
    "file": "index.js",
    "entry": "createPrompts",
    "sections": [
     "prompts"
    ],
    "default": true,
    "summary": "扫 Nous-prompts 私库，把这条会话选中的那条模板正文整段渲染进系统提示词（单选）",
    "needs": {
     "promptsDir": "提示词私库根目录（绝对路径，通常是 /opt/Nous-prompts）",
     "registry": "要把每条模板的空白名牌（prompt:<名>）装进去的 registry"
    },
    "category": [
     "观察",
     "Basics"
    ],
    "tracked": true
   },
   {
    "name": "skills",
    "version": "0.2.0",
    "file": "index.js",
    "entry": [
     "createSkills",
     "skillNamesOf"
    ],
    "sections": [
     "skills"
    ],
    "default": true,
    "summary": "扫 Nous-skills 仓库里的技能目录，把这条会话选中的渲染成 <available_skills>",
    "needs": {
     "dir": "技能仓库的根目录（绝对路径，通常是 /opt/Nous-skills）",
     "registry": "要把每条技能的空白名牌（skill:<名>）装进去的 registry"
    },
    "category": [
     "工具",
     "Basics"
    ],
    "tracked": true
   }
  ],
  "archived": [
   {
    "name": "demo",
    "version": "0.1.0",
    "summary": "凑数用的测试扩展：专门拿来练「删除本地 / 安装回来」",
    "category": [
     "Basics"
    ],
    "archivedAt": "2026-11-04T21:25:37+08:00"
   }
  ],
  "variants": {
   "123": [
    {
     "id": "123",
     "source": "local",
     "params": {},
     "dir": "/opt/nous-ext-123-123/123",
     "branch": "ext/123-123",
     "key": "123@123",
     "unmerged": 0,
     "remoteBranch": false,
     "installed": false,
     "load": {
      "state": "failed",
      "error": "Cannot find module '/opt/nous-ext-123-123/123/index.js' imported from /opt/Nous/content/new1/map/qrd3wh/server.js",
      "at": 1794813498717,
      "version": "0.1.0"
     }
    }
   ]
  },
  "load": {
   "123": {
    "state": "ok",
    "error": null,
    "at": 1794813498714,
    "version": "0.1.0"
   },
   "123@123": {
    "state": "failed",
    "error": "Cannot find module '/opt/nous-ext-123-123/123/index.js' imported from /opt/Nous/content/new1/map/qrd3wh/server.js",
    "at": 1794813498717,
    "version": "0.1.0"
   }
  },
  "local": {
   "_about": [
    "本机装卸清单：哪些扩展这台不装。不在 Nous-extend 仓库里，也不该被提交。"
   ],
   "deleted": {},
   "variants": {
    "123": {
     "123": {
      "dir": "/opt/nous-ext-123-123/123",
      "branch": "ext/123-123"
     }
    }
   },
   "core": [
    "nous",
    "skills",
    "prompts"
   ]
  },
  "nameplates": {
   "prefixes": [
    "skill:",
    "prompt:"
   ],
   "count": 11,
   "names": [
    "skill:ask-matt",
    "skill:code-review",
    "skill:deep-dive",
    "skill:incident-postmortem",
    "skill:scratch-notes",
    "skill:weekly-report",
    "prompt:demo-diagram",
    "prompt:demo-diary",
    "prompt:demo-weekly",
    "prompt:one-sentence",
    "prompt:smart-ass"
   ]
  },
  "live": {
   "nous": [
    "preamble",
    "cwd"
   ],
   "skills": [
    "skills"
   ],
   "prompts": [
    "prompts"
   ]
  },
  "files": {
   "skills": [
    {
     "path": "index.js",
     "kind": "javascript",
     "size": 18035,
     "mtime": 1793959405000
    },
    {
     "path": "nous.json",
     "kind": "json",
     "size": 531,
     "mtime": 1793956219000
    }
   ]
  },
  "text": {},
  "snippet": {
   "skills/index.js": "// createSkills：把 Nous-skills 里选中的技能渲染成 <available_skills>\nexport function createSkills({ dir, registry }) {\n  const skills = new Map();\n\n  const render = (names) => {\n    const picked = names.map((n) => skills.get(n)).filter(Boolean);\n    if (!picked.length) return \"\";\n    const body = picked.map((s) => `- ${s.name}: ${s.summary}`).join(\"\\n\");\n    return `<available_skills>\\n${body}\\n</available_skills>`;\n  };\n\n  return {\n    section: \"skills\",\n    render,\n    async load() { /* 扫目录、读 frontmatter */ },\n  };\n}\n\nexport const skillNamesOf = (list) => list.map((s) => `skill:${s.name}`);\n"
  }
 }
};
