/**
 * Keeps the screen on while a page is open (cook and prep modes).
 * Browsers may require a tap first, so `needsTap` tells the UI to offer one.
 */
export function useScreenAwake() {
  const lock = useWakeLock()
  const needsTap = ref(false)

  async function request() {
    if (!lock.isSupported.value) return
    try {
      await lock.request("screen")
      needsTap.value = !lock.isActive.value
    } catch {
      needsTap.value = true
    }
  }

  onMounted(request)
  onBeforeUnmount(() => {
    void lock.release()
  })

  return { isSupported: lock.isSupported, isActive: lock.isActive, needsTap, request }
}
