// Bun's bundler (and test runner) imports TOML as its parsed object.
declare module "*.toml" {
  const value: Record<string, unknown>
  export default value
}
