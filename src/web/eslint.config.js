import js from "@eslint/js";
import sonarjs from "eslint-plugin-sonarjs";
import unicorn from "eslint-plugin-unicorn";
import tseslint from "typescript-eslint";

const noMultiComp = {
  meta: {
    type: "suggestion",
    schema: [],
    messages: {
      multiComp: "Only one component per file. Move '{{name}}' to its own file.",
    },
  },
  create(context) {
    const components = [];
    const isPascalCase = (name) => /^[A-Z]/.test(name);
    const isForwardRef = (init) =>
      init.type === "CallExpression" &&
      init.callee.type === "Identifier" &&
      init.callee.name === "forwardRef";
    return {
      VariableDeclarator(node) {
        if (node.id.type !== "Identifier" || !isPascalCase(node.id.name)) return;
        if (!node.init) return;
        if (node.init.type === "ArrowFunctionExpression" || isForwardRef(node.init)) {
          components.push(node);
        }
      },
      "Program:exit"() {
        for (const node of components.slice(1)) {
          context.report({ node, messageId: "multiComp", data: { name: node.id.name } });
        }
      },
    };
  },
};

const noRawTextElements = {
  meta: {
    type: "suggestion",
    schema: [],
    messages: {
      rawTextElement: "Use <Typography> instead of raw <{{tag}}>. Import from '@/components/ui'.",
    },
  },
  create(context) {
    const BANNED_TAGS = new Set(["h1", "h2", "h3", "h4", "h5", "h6", "p"]);
    const filePath = context.filename || context.getFilename();
    const isTypographyFile = filePath.includes("typography");
    if (isTypographyFile) return {};
    return {
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier") return;
        if (!BANNED_TAGS.has(node.name.name)) return;
        context.report({
          node,
          messageId: "rawTextElement",
          data: { tag: node.name.name },
        });
      },
    };
  },
};

export default [
  {
    ignores: ["dist/**", "node_modules/**", "gen/**", "*.config.ts", "*.config.js"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommendedTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      local: {
        rules: { "no-multi-comp": noMultiComp, "no-raw-text-elements": noRawTextElements },
      },
      sonarjs,
      unicorn,
    },
    rules: {
      // --- Disable rules that overlap with Biome ---
      indent: "off",
      quotes: "off",
      semi: "off",
      "comma-dangle": "off",
      "no-unused-vars": "off",
      "sort-imports": "off",
      "no-multiple-empty-lines": "off",
      "eol-last": "off",

      // --- Component structure ---
      "local/no-multi-comp": "error",
      "local/no-raw-text-elements": "error",

      // --- Structural limits ---
      "max-lines": ["error", { max: 200, skipBlankLines: true, skipComments: true }],
      "max-depth": ["error", 2],
      "max-lines-per-function": ["error", { max: 40, skipBlankLines: true, skipComments: true }],

      // --- TypeScript-specific ---
      "@typescript-eslint/consistent-type-imports": ["error", { prefer: "type-imports" }],
      "@typescript-eslint/consistent-type-definitions": ["error", "interface"],
      "@typescript-eslint/prefer-readonly": "error",
      "@typescript-eslint/use-unknown-in-catch-callback-variable": "error",
      "func-style": ["error", "expression"],
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
        },
      ],
      "@typescript-eslint/naming-convention": [
        "error",
        { selector: "default", format: ["camelCase"], leadingUnderscore: "allow" },
        {
          selector: "variable",
          format: ["camelCase", "UPPER_CASE", "PascalCase"],
          leadingUnderscore: "allow",
          filter: {
            regex: "(Map|Object|String|Array|List|Set|Dict|Number|Boolean|Fn|Func|Callback)$",
            match: false,
          },
        },
        {
          selector: "function",
          format: ["camelCase", "PascalCase"],
          filter: {
            regex: "(Map|Object|String|Array|List|Set|Dict|Number|Boolean|Fn|Func|Callback)$",
            match: false,
          },
        },
        {
          selector: "parameter",
          format: ["camelCase"],
          leadingUnderscore: "allow",
          filter: {
            regex: "(Map|Object|String|Array|List|Set|Dict|Number|Boolean|Fn|Func|Callback)$",
            match: false,
          },
        },
        { selector: "typeLike", format: ["PascalCase"] },
        {
          selector: "objectLiteralProperty",
          format: null,
          filter: { regex: "^[a-z]+(_[a-z]+)+$", match: true },
        },
        {
          selector: "objectLiteralProperty",
          format: null,
          filter: { regex: "^(data|aria)-", match: true },
        },
      ],
      "@typescript-eslint/no-magic-numbers": [
        "error",
        {
          ignore: [0, 1, -1, 2, 6, 20, 76, 1.5, 1.75],
          ignoreEnums: true,
          ignoreNumericLiteralTypes: true,
          ignoreReadonlyClassProperties: true,
          ignoreDefaultValues: true,
          ignoreClassFieldInitialValues: true,
        },
      ],
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "never" }],
      "@typescript-eslint/no-floating-promises": "off",
      "@typescript-eslint/no-misused-promises": "error",

      // --- Sonarjs (complexity and duplication) ---
      "sonarjs/cognitive-complexity": ["error", 10],
      "sonarjs/no-duplicate-string": ["error", { threshold: 3 }],
      "sonarjs/no-identical-functions": "error",

      // --- Import hygiene ---
      "no-restricted-imports": [
        "error",
        {
          patterns: ["../*"],
        },
      ],

      // --- Export style + async style ---
      "no-restricted-syntax": [
        "error",
        {
          selector: "ExportNamedDeclaration[declaration.type='VariableDeclaration']",
          message: "Use `export { name }` at bottom of file instead of `export const`.",
        },
        {
          selector: "CallExpression[callee.property.name='then']",
          message: "Use async/await instead of .then(). Extract async function if needed.",
        },
        {
          selector: "CallExpression[callee.property.name='finally']",
          message: "Use try/finally with async/await instead of .finally().",
        },
      ],

      // --- Unicorn (patterns) ---
      "unicorn/no-nested-ternary": "error",
    },
  },
  {
    // Relax rules for test files
    files: ["**/*.test.ts", "**/*.test.tsx", "test/**/*.ts"],
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-magic-numbers": "off",
      "func-style": "off",
      "@typescript-eslint/no-floating-promises": "off",
      "@typescript-eslint/no-unsafe-assignment": "off",
      "@typescript-eslint/naming-convention": "off",
      "@typescript-eslint/consistent-type-definitions": "off",
      "@typescript-eslint/consistent-type-assertions": "off",
      "@typescript-eslint/prefer-readonly": "off",
      "@typescript-eslint/no-unsafe-argument": "off",
      "@typescript-eslint/no-unsafe-call": "off",
      "@typescript-eslint/no-unsafe-member-access": "off",
      "sonarjs/no-duplicate-string": "off",
      "sonarjs/cognitive-complexity": "off",
      "max-depth": "off",
      "max-lines-per-function": "off",
    },
  },
];
