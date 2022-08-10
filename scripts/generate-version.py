import os
import requests
import json

RELEASE_URL = 'https://api.github.com/repos/a5huynh/spyglass/releases'

DARWIN_x86 = 'darwin-x86_64'
DARWIN_ARM = 'darwin-aarch64'
LINUX_x86  = 'linux-x86_64'
WINDOWS_x86 = 'windows-x86_64'

def generate(assets):
    platforms = {
        DARWIN_x86: { 'signature': '', 'url': '' },
        DARWIN_ARM: { 'signature': '', 'url': '' },
        LINUX_x86: { 'signature': '', 'url': '' },
        WINDOWS_x86: { 'signature': '', 'url': '' },
    }


    for value in assets:
        name = value['name']
        if name.endswith('.tar.gz') or name.endswith('.zip'):
            dl_url = value['browser_download_url']
            if name == "Spyglass.app.tar.gz":
                platforms[DARWIN_x86]['url'] = dl_url
                platforms[DARWIN_ARM]['url'] = dl_url
            elif name.endswith('AppImage.tar.gz'):
                platforms[LINUX_x86]['url'] = dl_url
            elif name.endswith('.msi.zip'):
                platforms[WINDOWS_x86]['url'] = dl_url
        elif name.endswith('.sig'):
            dl_url = value['browser_download_url']
            sig = requests.get(dl_url).text

            if name.startswith('Spyglass.app'):
                platforms[DARWIN_x86]['signature'] = sig
                platforms[DARWIN_ARM]['signature'] = sig
            elif 'AppImage' in name:
                platforms[LINUX_x86]['signature'] = sig
            elif '.msi.zip' in name:
                platforms[WINDOWS_x86]['signature'] = sig
    return platforms

def main():
    res = requests.get(RELEASE_URL)
    latest = res.json()[0]

    print(f"Generating VERSION.json for {latest['tag_name']}")
    platforms = generate(latest['assets'])

    version = {
        'version': latest['tag_name'].replace('v20', ''),
        'notes': f"See full release notes here: https://github.com/a5huynh/spyglass/releases/tag/{latest['tag_name']}",
        'pub_date': latest['published_at'],
        'platforms': platforms
    }

    with open('VERSION.json', 'w') as hand:
        hand.write(json.dumps(version, indent=2))

if __name__ == '__main__':
    main()