export function init_env(domain, client_id, redirect_uri, audience) {
    window.AUTH0 = {
        domain,
        client_id,
        redirect_uri,
        audience
    };
}

async function get_client() {
    let client = await auth0
        .createAuth0Client({
            domain: window.AUTH0.domain,
            clientId: window.AUTH0.client_id,
            useRefreshTokens: true,
            cacheLocation: 'localstorage',
            authorizationParams: {
                audience: window.AUTH0.audience,
                redirect_uri: window.AUTH0.redirect_uri,
            },
        });

    return client;
}

export async function check_login() {
    return await get_client().then(async client => {
        const isAuthenticated = await client.isAuthenticated();
        if (isAuthenticated) {
            console.log('user is already logged in, refreshing state');
            const userProfile = await client.getUser();
            const token = await client.getTokenSilently();
            return { isAuthenticated, userProfile, token };
        }
        return null;
    });
}

export async function auth0_login() {
    await get_client().then(client => client.loginWithRedirect());
}

export async function auth0_logout() {
    await get_client().then(client => client.logout());
}

export async function handle_login_callback() {
    return await get_client()
        .then(async client => {
            await client.handleRedirectCallback();

            const isAuthenticated = await client.isAuthenticated();
            const userProfile = await client.getUser();
            const token = await client.getTokenSilently();
            return { isAuthenticated, userProfile, token };
        })
        .catch(err => {
            console.error('handle_login_callback: ', err);
            throw err;
        });
}