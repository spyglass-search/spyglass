module.exports = {
    content: ["./src/**/*.{html,js,rs}", "./*.html"],
    theme: {
        extend: {},
    },
    plugins: [
        require('@tailwindcss/forms')
    ],
}
