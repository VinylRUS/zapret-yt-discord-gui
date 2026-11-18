/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,js,ts}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        // Cloudflare One inspired palette
        brand: {
          50: "#f0f6ff",
          100: "#dbe9ff",
          200: "#bcd3ff",
          300: "#8eb3ff",
          400: "#5a8bff",
          500: "#2d63ff",
          600: "#1a47e6",
          700: "#1738c0",
          800: "#1a3399",
          900: "#1b3079",
        },
        ink: {
          900: "#0c0d10",
          800: "#131419",
          750: "#181a20",
          700: "#1d2027",
          600: "#262a33",
          500: "#3a4050",
          400: "#5a6075",
          300: "#8a90a3",
          200: "#b8bdcc",
          100: "#e2e5ec",
        },
      },
      fontFamily: {
        sans: [
          "Inter",
          "Segoe UI",
          "system-ui",
          "-apple-system",
          "sans-serif",
        ],
        mono: ["JetBrains Mono", "Cascadia Code", "Consolas", "monospace"],
      },
      boxShadow: {
        card: "0 1px 2px rgba(0,0,0,.08), 0 4px 16px rgba(0,0,0,.06)",
        "card-lg": "0 8px 32px rgba(0,0,0,.18)",
      },
      borderRadius: {
        xl: "12px",
        "2xl": "16px",
      },
    },
  },
  plugins: [],
};
