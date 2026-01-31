#!/usr/bin/env node
/**
 * VSCode 拡張パッケージ生成ツール
 * 
 * 使用方法:
 *   node build-vscode-ext.js
 * 
 * 出力:
 *   dist/nospace-lang/ - VSCode 拡張ディレクトリ
 */

const fs = require('fs');
const path = require('path');

const DIST_DIR = path.join(__dirname, 'dist', 'nospace-lang');
const SYNTAXES_DIR = path.join(DIST_DIR, 'syntaxes');

// 設定
const config = {
  name: 'nospace-lang',
  displayName: 'nospace Language',
  description: 'Syntax highlighting for the nospace programming language',
  version: '0.1.0',
  publisher: 'nospace',
  engines: {
    vscode: '^1.80.0'
  },
  categories: ['Programming Languages'],
  repository: {
    type: 'git',
    url: 'https://github.com/buyoh/nospace20'
  }
};

console.log('=== Building VSCode Extension ===\n');

// 1. ディレクトリ作成
if (!fs.existsSync(DIST_DIR)) {
  fs.mkdirSync(DIST_DIR, { recursive: true });
}
if (!fs.existsSync(SYNTAXES_DIR)) {
  fs.mkdirSync(SYNTAXES_DIR, { recursive: true });
}
console.log('✓ Created directories');

// 2. package.json 生成
const packageJson = {
  ...config,
  contributes: {
    languages: [
      {
        id: 'nospace',
        aliases: ['nospace', 'Nospace'],
        extensions: ['.ns'],
        configuration: './language-configuration.json'
      }
    ],
    grammars: [
      {
        language: 'nospace',
        scopeName: 'source.nospace',
        path: './syntaxes/nospace.tmLanguage.json'
      }
    ]
  }
};

fs.writeFileSync(
  path.join(DIST_DIR, 'package.json'),
  JSON.stringify(packageJson, null, 2)
);
console.log('✓ Generated package.json');

// 3. language-configuration.json 生成
const languageConfig = {
  comments: {
    blockComment: ['#', '#']
  },
  brackets: [
    ['{', '}'],
    ['[', ']'],
    ['(', ')']
  ],
  autoClosingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: "'", close: "'", notIn: ['string', 'comment'] },
    { open: '#', close: '#', notIn: ['string', 'comment'] }
  ],
  surroundingPairs: [
    ['{', '}'],
    ['[', ']'],
    ['(', ')'],
    ["'", "'"]
  ],
  folding: {
    markers: {
      start: '^\\s*\\{',
      end: '^\\s*\\}'
    }
  },
  wordPattern: '[a-zA-Z_][a-zA-Z0-9_]*'
};

fs.writeFileSync(
  path.join(DIST_DIR, 'language-configuration.json'),
  JSON.stringify(languageConfig, null, 2)
);
console.log('✓ Generated language-configuration.json');

// 4. TextMate Grammar をコピー
const grammarSrc = path.join(__dirname, '..', '..', 'syntaxes', 'nospace.tmLanguage.json');
const grammarDst = path.join(SYNTAXES_DIR, 'nospace.tmLanguage.json');

if (fs.existsSync(grammarSrc)) {
  fs.copyFileSync(grammarSrc, grammarDst);
  console.log('✓ Copied nospace.tmLanguage.json');
} else {
  console.log('✗ Grammar file not found:', grammarSrc);
  process.exit(1);
}

// 5. README.md 生成
const readme = `# nospace Language Support

Syntax highlighting for the nospace programming language.

## Features

- Syntax highlighting for \`.ns\` files
- Comment support (\`# ... #\`)
- Bracket matching
- Auto-closing pairs

## Installation

### From VSIX

1. Download the \`.vsix\` file
2. Open VSCode
3. Press \`Ctrl+Shift+P\` and run "Extensions: Install from VSIX..."
4. Select the downloaded file

### Development Mode

1. Copy this folder to \`~/.vscode/extensions/nospace-lang\`
2. Restart VSCode

## Syntax

\`\`\`nospace
# Example program #
func: main() {
  let: x;
  x = 42;
  if: x - 40 {
    __clog(x);
  };
  return: 0;
}
\`\`\`

## License

MIT
`;

fs.writeFileSync(path.join(DIST_DIR, 'README.md'), readme);
console.log('✓ Generated README.md');

// 6. .vscodeignore 生成
const vscodeignore = `.git
.gitignore
*.md
!README.md
`;

fs.writeFileSync(path.join(DIST_DIR, '.vscodeignore'), vscodeignore);
console.log('✓ Generated .vscodeignore');

// サマリー
console.log('\n=== Build Complete ===');
console.log(`Output: ${DIST_DIR}`);
console.log('\nTo install in development mode:');
console.log(`  cp -r ${DIST_DIR} ~/.vscode/extensions/`);
console.log('\nTo package as VSIX:');
console.log('  npm install -g @vscode/vsce');
console.log(`  cd ${DIST_DIR} && vsce package`);
