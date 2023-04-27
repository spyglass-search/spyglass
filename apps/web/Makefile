.PHONY: deploy

deploy:
	npm run build
	trunk build --release
	aws s3 cp --recursive dist s3://search.spyglass.fyi
