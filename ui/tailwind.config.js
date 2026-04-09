/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./**/*.rs", "../prerender/**/*.rs"],
  darkMode: 'class',
  theme: {
    extend: {},
  },
  plugins: [
    require('@tailwindcss/typography'),
  ],
}
