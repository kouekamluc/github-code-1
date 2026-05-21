module.exports = {
  content: [
    './static/**/*.html',
    './static/**/*.js',
  ],
  theme: {
    extend: {
      colors: {
        ink: '#142033',
        muted: '#64748b',
        night: '#0d1522',
        pulse: '#0f766e',
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', 'Segoe UI', 'sans-serif'],
      },
    },
  },
  plugins: [],
};
