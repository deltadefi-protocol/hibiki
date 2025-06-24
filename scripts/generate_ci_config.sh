#!/bin/bash

# Define the environment file
ENV_FILE=".env"

# Check if argument is provided
if [ -z "$1" ]; then
    echo "No environment specified. Please specify 'dev' or 'prod'."
    exit 1
fi

# Define the environment suffix based on the argument
if [ "$1" = "dev" ]; then
    ENV_SUFFIX="_DEV"
elif [ "$1" = "prod" ]; then
    ENV_SUFFIX="_PROD"
else
    echo "Invalid environment specified. Please specify 'dev' or 'prod'."
    exit 1
fi

# Read the environment file line by line
while IFS= read -r line
do
    # Skip empty lines and lines starting with a comment
    if [ -z "$line" ] || [[ $line == \#* ]]; then
        continue
    fi
    # Split the line into name and value
    IFS='=' read -ra PARTS <<< "$line"
    NAME=${PARTS[0]}

    # Output the line in the desired format
    echo "echo \"export $NAME=\$$NAME$ENV_SUFFIX\" >> \$BASH_ENV" > /dev/stdout
done < "$ENV_FILE"