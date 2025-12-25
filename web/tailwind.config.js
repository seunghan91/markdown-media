/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './app/frontend/**/*.{svelte,js,ts}',
    './app/views/**/*.html.erb',
  ],
  theme: {
    extend: {
      colors: {
        primary: {
          50: '#f0f9ff',
          100: '#e0f2fe',
          500: '#0ea5e9',
          600: '#0284c7',
          700: '#0369a1',
        },
      },
    },
  },
  plugins: [],
}
