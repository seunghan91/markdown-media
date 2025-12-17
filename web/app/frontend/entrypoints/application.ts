import { createInertiaApp } from '@inertiajs/svelte'
import '../styles/application.css'

const pages = (import.meta as any).glob('../pages/**/*.svelte', { eager: true }) as Record<string, any>

createInertiaApp({
  resolve: (name: string) => {
    const key = `../pages/${name}.svelte`
    if (!(key in pages)) {
      throw new Error(`Page not found: ${name}`)
    }
    return pages[key]
  },
  setup({ el, App, props }: any) {
    new App({ target: el, props })
  }
})
