interface Viewed {
  id: number
  title: string
  image: string | null
}

/** Last few recipes opened on this device, for "pick up where you left off". */
export function useRecentlyViewed() {
  const list = useLocalStorage<Viewed[]>("jtr:recent", [])
  function remember(recipe: Viewed) {
    list.value = [
      { id: recipe.id, title: recipe.title, image: recipe.image },
      ...list.value.filter((r) => r.id !== recipe.id),
    ].slice(0, 6)
  }
  function forget(id: number) {
    list.value = list.value.filter((r) => r.id !== id)
  }
  return { list, remember, forget }
}
