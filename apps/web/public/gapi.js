let tokenClient;
let accessToken = null;
let CLIENT_ID = null;
let API_KEY = null;

export function init_gapi(client_id, api_key) {
    gapi.load('picker');
    CLIENT_ID = client_id;
    API_KEY = api_key;
    tokenClient = google.accounts.oauth2.initTokenClient({
        client_id: CLIENT_ID,
        scope: 'https://www.googleapis.com/auth/drive.file',
        callback: '', // defined later
    });
}

function pickerCallback(data) {
    let file_id = null;
    if (data[google.picker.Response.ACTION] == google.picker.Action.PICKED) {
        let doc = data[google.picker.Response.DOCUMENTS][0];
        file_id = doc[google.picker.Document.ID];
    }
    return file_id;
}

export function create_picker(callback) {
    const showPicker = () => {
        let view = new google.picker.DocsView()
            .setIncludeFolders(true)
            .setSelectFolderEnabled(true);

        const picker = new google.picker.PickerBuilder()
            .addView(view)
            .setOAuthToken(accessToken)
            .setDeveloperKey(API_KEY)
            .setCallback((data) => {
                let result = pickerCallback(data);
                if (result) {
                    callback(accessToken, result)
                };
            })
            .build();
            picker.setVisible(true);
    }

    // Request an access token.
    tokenClient.callback = async (response) => {
        if (response.error !== undefined) {
            throw (response);
        }
        accessToken = response.access_token;
        showPicker();
    };

    if (accessToken === null) {
        // Prompt the user to select a Google Account and ask for consent to share their data
        // when establishing a new session.
        tokenClient.requestAccessToken({prompt: 'consent'});
    } else {
        // Skip display of account chooser and consent dialog for an existing session.
        tokenClient.requestAccessToken({prompt: ''});
    }
}