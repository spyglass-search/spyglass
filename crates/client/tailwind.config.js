module.exports = {
    content: ["./src/**/*.{html,js,rs}", "./*.html"],
    theme: {
        extend: {
            keyframes: {
                wiggle: {
                  '0%, 100%': { transform: 'rotate(-6deg)' },
                  '50%': { transform: 'rotate(6deg)' },
                }
            },
            animation: {
                'wiggle-short': 'wiggle 1s ease-in-out 10',
                'wiggle': 'wiggle 1s ease-in-out infinite',
            }
        },
    },
    plugins: [
        require('@tailwindcss/forms')({ strategy: 'class' })
    ],
}
