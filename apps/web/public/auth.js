export function init_env(domain, client_id, redirect_uri) {
    window.AUTH0 = {
        domain,
        client_id,
        redirect_uri
    };
}

async function get_client() {
    let client = await auth0
        .createAuth0Client({
            domain: window.AUTH0.domain,
            clientId: window.AUTH0.client_id,
            authorizationParams: {
                redirect_uri: window.AUTH0.redirect_uri,
            },
        });

    return client;
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
            return { isAuthenticated, userProfile };
        })
        .catch(err => console.log(err));
}