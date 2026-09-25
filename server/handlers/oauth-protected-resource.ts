// Served at /.well-known/oauth-protected-resource[/mcp] (registered in nuxt.config)
export default defineEventHandler((event) => {
  setHeader(event, "access-control-allow-origin", "*")
  return protectedResourceMetadata(publicOrigin(event))
})
