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
        fontSize: {
            // 10px
            xs: '0.625rem',
            // 12px
            sm: '0.75rem',
            // 14px
            base: '0.875rem',
            // 16px
            lg: '1rem',
            xl: '1.125rem',
            '2xl': '1.25rem',
            '3xl': '1.5rem',
            '4xl': '1.875rem',
            '5xl': '2rem',
        }
    },
    plugins: [
        require('@tailwindcss/forms')({ strategy: 'class' })
    ],
}
