import boto3
from botocore.exceptions import ClientError
import os
import json

BUCKET = 'update.spyglass.fyi'
FILE_NAME = 'VERSION.json'


def upload_file(file_name, bucket, object_name=None):
    """Upload VERSION.json file to S3 """

    # If S3 object_name was not specified, use file_name
    if object_name is None:
        object_name = os.path.basename(file_name)

    # Upload the file
    s3_client = boto3.client('s3')
    try:
        response = s3_client.upload_file(file_name, bucket, object_name)
    except ClientError as e:
        print(f"Unable to upload to S3: {e}")
        return False

    return True

def main():
    version = json.load(open(FILE_NAME))
    print(f"Uploading {version['version']}")
    if upload_file(FILE_NAME, BUCKET):
        print("Upload successful! You'll need to manually create the invalidation on Cloudfront now")

if __name__ == '__main__':
    main()