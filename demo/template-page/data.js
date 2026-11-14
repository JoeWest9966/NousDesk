window.DATA = {
 "sections": {
  "Session-Template": {
   "section": "Session-Template",
   "title": "会话模板",
   "root": "/opt/Nous-Template/Session-Template",
   "repo": {
    "head": "9e4ccd1300c60f154e80fac8d5e5327edc54e87d",
    "dirty": true,
    "ahead": 0,
    "behind": 0,
    "remote": "https://github.com/JoeWest9966/Nous-Template",
    "error": null
   },
   "presets": [
    {
     "name": "deep-dive",
     "summary": "一个技术问题挖到底：先列假设，再找反证，最后给结论和代价",
     "description": "钉了模型和档位。这种活最怕中途换模型 —— 思路断了还得重来。",
     "category": [
      "工程",
      "写作"
     ],
     "spec": {
      "v": 1,
      "name": "deep-dive",
      "summary": "一个技术问题挖到底：先列假设，再找反证，最后给结论和代价",
      "category": [
       "工程",
       "写作"
      ],
      "description": "钉了模型和档位。这种活最怕中途换模型 —— 思路断了还得重来。",
      "model": {
       "provider": "volc-ark",
       "modelId": "deepseek-v4-flash-260425"
      },
      "thinkingLevel": "medium",
      "skills": [
       "deep-dive"
      ],
      "template": "smart-ass",
      "readOnly": false
     },
     "specError": null,
     "tracked": true,
     "dir": "/opt/Nous-Template/Session-Template/deep-dive",
     "resolved": {
      "name": "deep-dive",
      "model": {
       "state": "ok",
       "ref": "volc-ark/deepseek-v4-flash-260425",
       "value": "volc-ark/deepseek-v4-flash-260425",
       "thinkingLevels": [
        "off",
        "minimal",
        "low",
        "medium",
        "high"
       ]
      },
      "thinking": {
       "state": "ok",
       "ref": "medium",
       "value": "medium"
      },
      "skills": [
       {
        "state": "ok",
        "ref": "deep-dive"
       }
      ],
      "extensions": [],
      "template": {
       "state": "ok",
       "ref": "smart-ass",
       "value": "smart-ass",
       "mode": "picked"
      },
      "readOnly": false,
      "cwd": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "machine": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "absence": {
       "skills": false,
       "extensions": true,
       "template": false,
       "model": false,
       "thinkingLevel": false,
       "readOnly": false,
       "cwd": true,
       "machine": true
      }
     }
    },
    {
     "name": "read-only-review",
     "summary": "只读审查：能读能跑，不能写盘 —— 提意见的会话不该顺手改代码",
     "description": "模板跟随仓库默认（settings.json 的 default）。改这个默认值，这条预设的行为会跟着变 —— 这是故意的。",
     "category": [
      "工程"
     ],
     "spec": {
      "v": 1,
      "name": "read-only-review",
      "summary": "只读审查：能读能跑，不能写盘 —— 提意见的会话不该顺手改代码",
      "category": [
       "工程"
      ],
      "description": "模板跟随仓库默认（settings.json 的 default）。改这个默认值，这条预设的行为会跟着变 —— 这是故意的。",
      "skills": [
       "code-review"
      ],
      "extensions": [
       "nous"
      ],
      "readOnly": true
     },
     "specError": null,
     "tracked": true,
     "dir": "/opt/Nous-Template/Session-Template/read-only-review",
     "resolved": {
      "name": "read-only-review",
      "model": {
       "state": "auto",
       "ref": null,
       "value": "volc-ark/deepseek-v4-flash-260425",
       "thinkingLevels": [
        "off",
        "minimal",
        "low",
        "medium",
        "high"
       ]
      },
      "thinking": {
       "state": "ok",
       "ref": null,
       "value": null
      },
      "skills": [
       {
        "state": "ok",
        "ref": "code-review"
       }
      ],
      "extensions": [
       {
        "state": "ok",
        "ref": "nous"
       }
      ],
      "template": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "readOnly": true,
      "cwd": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "machine": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "absence": {
       "skills": false,
       "extensions": false,
       "template": true,
       "model": true,
       "thinkingLevel": true,
       "readOnly": false,
       "cwd": true,
       "machine": true
      }
     }
    },
    {
     "name": "scratch",
     "summary": "白板：一条模板都不要、一条技能都不要，从零开始聊",
     "description": "template 是 null 不是缺席 —— 「明确不要」和「没说」是两件事，混了就会被仓库默认悄悄塞一条回来。",
     "category": [],
     "spec": {
      "v": 1,
      "name": "scratch",
      "summary": "白板：一条模板都不要、一条技能都不要，从零开始聊",
      "category": [],
      "description": "template 是 null 不是缺席 —— 「明确不要」和「没说」是两件事，混了就会被仓库默认悄悄塞一条回来。",
      "skills": [],
      "template": null,
      "readOnly": false
     },
     "specError": null,
     "tracked": true,
     "dir": "/opt/Nous-Template/Session-Template/scratch",
     "resolved": {
      "name": "scratch",
      "model": {
       "state": "auto",
       "ref": null,
       "value": "volc-ark/deepseek-v4-flash-260425",
       "thinkingLevels": [
        "off",
        "minimal",
        "low",
        "medium",
        "high"
       ]
      },
      "thinking": {
       "state": "ok",
       "ref": null,
       "value": null
      },
      "skills": [],
      "extensions": [],
      "template": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "none"
      },
      "readOnly": false,
      "cwd": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "machine": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "absence": {
       "skills": false,
       "extensions": true,
       "template": false,
       "model": true,
       "thinkingLevel": true,
       "readOnly": false,
       "cwd": true,
       "machine": true
      }
     }
    },
    {
     "name": "weekly-report",
     "summary": "把一周的提交、聊天、会议整理成一份能直接发出去的周报",
     "description": "周五收工用。模板自带口径，进去直接贴素材，不用再解释一遍要什么格式。",
     "category": [
      "写作",
      "写作/周报"
     ],
     "spec": {
      "v": 1,
      "name": "weekly-report",
      "summary": "把一周的提交、聊天、会议整理成一份能直接发出去的周报",
      "category": [
       "写作",
       "写作/周报"
      ],
      "description": "周五收工用。模板自带口径，进去直接贴素材，不用再解释一遍要什么格式。",
      "skills": [
       "weekly-report"
      ],
      "template": "demo-weekly",
      "readOnly": false
     },
     "specError": null,
     "tracked": true,
     "dir": "/opt/Nous-Template/Session-Template/weekly-report",
     "resolved": {
      "name": "weekly-report",
      "model": {
       "state": "auto",
       "ref": null,
       "value": "volc-ark/deepseek-v4-flash-260425",
       "thinkingLevels": [
        "off",
        "minimal",
        "low",
        "medium",
        "high"
       ]
      },
      "thinking": {
       "state": "ok",
       "ref": null,
       "value": null
      },
      "skills": [
       {
        "state": "ok",
        "ref": "weekly-report"
       }
      ],
      "extensions": [],
      "template": {
       "state": "ok",
       "ref": "demo-weekly",
       "value": "demo-weekly",
       "mode": "picked"
      },
      "readOnly": false,
      "cwd": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "machine": {
       "state": "ok",
       "ref": null,
       "value": null,
       "mode": "default"
      },
      "absence": {
       "skills": false,
       "extensions": true,
       "template": false,
       "model": true,
       "thinkingLevel": true,
       "readOnly": false,
       "cwd": true,
       "machine": true
      }
     }
    }
   ],
   "archived": [],
   "categories": [
    "工程",
    "写作",
    "写作/周报",
    "Basics"
   ]
  },
  "Ultimate-Template": {
   "section": "Ultimate-Template",
   "title": "终极模板",
   "root": "/opt/Nous-Template/Ultimate-Template",
   "repo": {
    "head": "9e4ccd1300c60f154e80fac8d5e5327edc54e87d",
    "dirty": true,
    "ahead": 0,
    "behind": 0,
    "remote": "https://github.com/JoeWest9966/Nous-Template",
    "error": null
   },
   "presets": [
    {
     "name": "test-probe",
     "summary": "端到端验证用，验完就删",
     "description": "",
     "category": [],
     "spec": {
      "v": 1,
      "name": "test-probe",
      "summary": "端到端验证用，验完就删",
      "category": [],
      "trigger": "single-shot",
      "params": {
       "epoch": 1800000060
      },
      "session": "scratch",
      "enabled": true
     },
     "specError": null,
     "tracked": false,
     "dir": "/opt/Nous-Template/Ultimate-Template/test-probe",
     "resolved": {
      "name": "test-probe",
      "trigger": {
       "state": "ok",
       "ref": "single-shot",
       "value": "指定的那一分钟里发起一次 —— 过时不补，跑过不再跑",
       "version": "0.1.0",
       "params": [
        {
         "name": "epoch",
         "state": "ok",
         "value": 1800000060,
         "label": "什么时候",
         "problem": ""
        }
       ],
       "unknown": []
      },
      "session": {
       "state": "ok",
       "ref": "scratch"
      },
      "enabled": true
     }
    }
   ],
   "archived": [],
   "categories": [
    "日常",
    "Basics"
   ]
  }
 },
 "triggers": [
  {
   "name": "single-shot",
   "summary": "指定的那一分钟里发起一次 —— 过时不补，跑过不再跑",
   "category": [
    "时间",
    "时间/一次"
   ],
   "version": "0.1.0",
   "params": [
    {
     "name": "epoch",
     "type": "epoch",
     "label": "什么时候",
     "required": true,
     "help": "精确到分钟：落在这一分钟里就发起一次。页面上用本地时间挑，存进去是 UTC 秒。"
    }
   ]
  }
 ],
 "pending": {
  "head": "9e4ccd1300c60f154e80fac8d5e5327edc54e87d",
  "dirty": true,
  "ahead": 0,
  "behind": 0,
  "remote": "https://github.com/JoeWest9966/Nous-Template",
  "units": [
   {
    "key": "__categories",
    "kind": "categories",
    "name": null,
    "tracked": true,
    "files": [
     {
      "path": "categories.json",
      "status": "M",
      "kind": "manifest",
      "deleted": false,
      "added": 2,
      "removed": 2
     }
    ],
    "added": 2,
    "removed": 2,
    "code": false,
    "hasDeleted": false
   },
   {
    "key": "deep-dive",
    "kind": "preset",
    "name": "deep-dive",
    "tracked": true,
    "files": [
     {
      "path": "deep-dive/preset.json",
      "status": "M",
      "kind": "manifest",
      "deleted": false,
      "added": 11,
      "removed": 3
     }
    ],
    "added": 11,
    "removed": 3,
    "code": false,
    "hasDeleted": false
   }
  ],
  "aheadCommits": []
 },
 "diffs": {
  "deep-dive/preset.json": {
   "path": "deep-dive/preset.json",
   "before": "{\n  \"v\": 1,\n  \"name\": \"deep-dive\",\n  \"summary\": \"一个技术问题挖到底：先列假设，再找反证，最后给结论和代价\",\n  \"category\": [\"工程\"],\n  \"description\": \"钉了模型和档位。这种活最怕中途换模型 —— 思路断了还得重来。\",\n  \"model\": { \"provider\": \"volc-ark\", \"modelId\": \"deepseek-v4-flash-260425\" },\n  \"thinkingLevel\": \"medium\",\n  \"skills\": [\"deep-dive\"],\n  \"template\": \"smart-ass\",\n  \"readOnly\": false\n}\n",
   "after": "{\n  \"v\": 1,\n  \"name\": \"deep-dive\",\n  \"summary\": \"一个技术问题挖到底：先列假设，再找反证，最后给结论和代价\",\n  \"category\": [\n    \"工程\",\n    \"写作\"\n  ],\n  \"description\": \"钉了模型和档位。这种活最怕中途换模型 —— 思路断了还得重来。\",\n  \"model\": {\n    \"provider\": \"volc-ark\",\n    \"modelId\": \"deepseek-v4-flash-260425\"\n  },\n  \"thinkingLevel\": \"medium\",\n  \"skills\": [\n    \"deep-dive\"\n  ],\n  \"template\": \"smart-ass\",\n  \"readOnly\": false\n}\n"
  },
  "categories.json": {
   "path": "categories.json",
   "before": "{\n  \"v\": 1,\n  \"categories\": [\n    \"Basics\",\n    \"工程\",\n    \"写作\",\n    \"写作/周报\"\n  ]\n}\n",
   "after": "{\n  \"v\": 1,\n  \"categories\": [\n    \"工程\",\n    \"写作\",\n    \"写作/周报\",\n    \"Basics\"\n  ]\n}\n"
  },
  "sample": {
   "path": "scratch/preset.json",
   "before": "{\n  \"v\": 1,\n  \"name\": \"scratch\",\n  \"summary\": \"白板：一条模板都不要、一条技能都不要\",\n  \"category\": [],\n  \"model\": { \"provider\": \"volc-ark\", \"modelId\": \"deepseek-v4-flash-260425\" },\n  \"thinkingLevel\": \"medium\",\n  \"skills\": [\"scratch\"],\n  \"template\": \"smart-ass\",\n  \"readOnly\": false\n}\n",
   "after": "{\n  \"v\": 1,\n  \"name\": \"scratch\",\n  \"summary\": \"白板：一条模板都不要、一条技能都不要\",\n  \"category\": [\"Basics\"],\n  \"model\": null,\n  \"thinkingLevel\": \"high\",\n  \"skills\": [],\n  \"template\": null,\n  \"readOnly\": true\n}\n",
   "synthetic": true
  }
 },
 "pendingUT": {
  "head": "9e4ccd1300c60f154e80fac8d5e5327edc54e87d",
  "dirty": true,
  "ahead": 0,
  "behind": 0,
  "remote": "https://github.com/JoeWest9966/Nous-Template",
  "units": [
   {
    "key": "test-probe",
    "kind": "preset",
    "name": "test-probe",
    "tracked": false,
    "files": [
     {
      "path": "test-probe/",
      "status": "??",
      "kind": "other",
      "deleted": false,
      "added": 0,
      "removed": 0
     }
    ],
    "added": 0,
    "removed": 0,
    "code": false,
    "hasDeleted": false
   }
  ],
  "aheadCommits": []
 }
};
