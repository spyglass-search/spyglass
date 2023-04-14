.PHONY: deploy

deploy:
	trunk build --release
	aws s3 cp --recursive dist s3://app.spyglass.fyi
