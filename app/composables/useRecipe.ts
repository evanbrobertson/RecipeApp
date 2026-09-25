/** Loads a recipe by the current route's :id (shared by view, cook and prep pages). */
export function useRecipe() {
  const route = useRoute()
  const id = computed(() => Number(route.params.id))
  const {
    data: recipe,
    status,
    error,
  } = useFetch(() => `/api/recipes/${id.value}`, {
    key: `recipe-${id.value}`,
  })
  return { id, recipe, status, error }
}
