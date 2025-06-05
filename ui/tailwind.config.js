/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./**/*.rs"],
  darkMode: 'class',
  theme: {
    extend: {},
  },
  plugins: [
    require('@tailwindcss/typography'),
  ],
}
