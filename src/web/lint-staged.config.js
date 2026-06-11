export default {
  "*.{ts,tsx}": (files) => {
    const filtered = files.filter((f) => !f.includes("/gen/"));
    if (filtered.length === 0) return [];
    return [`biome check --write ${filtered.join(" ")}`, `eslint --fix ${filtered.join(" ")}`];
  },
  "*.{js,jsx}": ["biome check --write"],
  "*.json !*-lock.json": ["biome check --write"],
};
