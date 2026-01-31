#!/usr/bin/env node
/**
 * TextMate Grammar 検証ツール
 * 
 * 使用方法:
 *   node validate-tmgrammar.js [grammar-file] [test-file]
 *   node validate-tmgrammar.js ../../syntaxes/nospace.tmLanguage.json
 *   node validate-tmgrammar.js ../../syntaxes/nospace.tmLanguage.json ../../resources/tests/passes/c000.ns
 */

const fs = require('fs');
const path = require('path');

const grammarPath = process.argv[2] || '../../syntaxes/nospace.tmLanguage.json';
const testFilePath = process.argv[3];

console.log('=== TextMate Grammar Validation ===\n');

// 1. JSON として読み込み
let grammar;
try {
  const content = fs.readFileSync(path.resolve(__dirname, grammarPath), 'utf8');
  grammar = JSON.parse(content);
  console.log('✓ Valid JSON');
} catch (e) {
  console.error('✗ Invalid JSON:', e.message);
  process.exit(1);
}

// 2. 必須フィールドの確認
const requiredFields = ['scopeName', 'patterns'];
let hasError = false;

for (const field of requiredFields) {
  if (!grammar[field]) {
    console.error(`✗ Missing required field: ${field}`);
    hasError = true;
  } else {
    console.log(`✓ ${field}: ${typeof grammar[field] === 'string' ? grammar[field] : 'present'}`);
  }
}

if (hasError) {
  process.exit(1);
}

// 3. Repository の確認
if (grammar.repository) {
  const keys = Object.keys(grammar.repository);
  console.log(`✓ repository: ${keys.length} entries`);
  console.log(`  [${keys.join(', ')}]`);
}

// 4. パターンの検証
console.log('\n--- Pattern Analysis ---');

function extractPatterns(obj, depth = 0) {
  const patterns = [];
  
  if (obj.match) patterns.push({ type: 'match', regex: obj.match, name: obj.name });
  if (obj.begin) patterns.push({ type: 'begin', regex: obj.begin, name: obj.name });
  if (obj.end) patterns.push({ type: 'end', regex: obj.end, name: obj.name });
  
  if (obj.patterns) {
    for (const p of obj.patterns) {
      patterns.push(...extractPatterns(p, depth + 1));
    }
  }
  
  if (obj.repository) {
    for (const key of Object.keys(obj.repository)) {
      patterns.push(...extractPatterns(obj.repository[key], depth + 1));
    }
  }
  
  return patterns;
}

const allPatterns = extractPatterns(grammar);
console.log(`Total patterns: ${allPatterns.length}`);

// 5. 正規表現の検証
console.log('\n--- Regex Validation ---');
let regexErrors = 0;

for (const p of allPatterns) {
  try {
    // JavaScript の正規表現として検証（Oniguruma とは異なるが基本チェック）
    new RegExp(p.regex);
  } catch (e) {
    // Oniguruma 固有の構文はエラーになる可能性がある
    // 一般的なエラーのみ報告
    if (!p.regex.includes('(?<') && !p.regex.includes('\\p{')) {
      console.log(`  Warning: ${p.type} pattern may have issues`);
      console.log(`    regex: ${p.regex}`);
      console.log(`    error: ${e.message}`);
      regexErrors++;
    }
  }
}

if (regexErrors === 0) {
  console.log('✓ All regex patterns look valid');
}

// 6. include 参照の検証
console.log('\n--- Include Reference Check ---');

function extractIncludes(obj) {
  const includes = [];
  
  if (obj.include) {
    includes.push(obj.include);
  }
  
  if (obj.patterns) {
    for (const p of obj.patterns) {
      includes.push(...extractIncludes(p));
    }
  }
  
  if (obj.repository) {
    for (const key of Object.keys(obj.repository)) {
      includes.push(...extractIncludes(obj.repository[key]));
    }
  }
  
  return includes;
}

const includes = extractIncludes(grammar);
const repoKeys = new Set(Object.keys(grammar.repository || {}));
let includeErrors = 0;

for (const inc of includes) {
  if (inc.startsWith('#')) {
    const refName = inc.slice(1);
    if (!repoKeys.has(refName)) {
      console.log(`✗ Undefined reference: ${inc}`);
      includeErrors++;
    }
  }
}

if (includeErrors === 0) {
  console.log(`✓ All ${includes.length} includes are valid`);
}

// 7. テストファイルでのトークン化（オプション）
if (testFilePath) {
  console.log('\n--- Tokenization Test ---');
  console.log(`Test file: ${testFilePath}`);
  
  // vscode-textmate が利用可能かチェック
  try {
    const vsctm = require('vscode-textmate');
    const onig = require('vscode-oniguruma');
    
    const wasmPath = path.join(__dirname, 'node_modules/vscode-oniguruma/release/onig.wasm');
    if (!fs.existsSync(wasmPath)) {
      console.log('Note: Run `npm install` to enable tokenization test');
    } else {
      // 非同期で実行
      (async () => {
        const wasmBin = fs.readFileSync(wasmPath).buffer;
        await onig.loadWASM(wasmBin);
        
        const registry = new vsctm.Registry({
          onigLib: Promise.resolve({
            createOnigScanner: (patterns) => new onig.OnigScanner(patterns),
            createOnigString: (s) => new onig.OnigString(s)
          }),
          loadGrammar: async () => grammar
        });
        
        const g = await registry.loadGrammar(grammar.scopeName);
        const testContent = fs.readFileSync(path.resolve(__dirname, testFilePath), 'utf8');
        const lines = testContent.split('\n');
        
        console.log('\nTokens:');
        let ruleStack = vsctm.INITIAL;
        for (let i = 0; i < Math.min(lines.length, 20); i++) {
          const line = lines[i];
          const result = g.tokenizeLine(line, ruleStack);
          console.log(`Line ${i + 1}: ${line}`);
          for (const token of result.tokens) {
            console.log(`  [${token.startIndex}-${token.endIndex}] ${token.scopes.join(' ')}`);
          }
          ruleStack = result.ruleStack;
        }
      })();
    }
  } catch (e) {
    console.log('Note: Run `npm install` to enable tokenization test');
  }
}

// サマリー
console.log('\n=== Summary ===');
if (regexErrors === 0 && includeErrors === 0) {
  console.log('✓ Grammar validation passed!');
  process.exit(0);
} else {
  console.log(`✗ Found ${regexErrors + includeErrors} issue(s)`);
  process.exit(1);
}
