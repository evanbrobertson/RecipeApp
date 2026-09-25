// Served at /.well-known/oauth-authorization-server (registered in nuxt.config)
export default defineEventHandler((event) => {
  setHeader(event, "access-control-allow-origin", "*")
  return authServerMetadata(publicOrigin(event))
})
