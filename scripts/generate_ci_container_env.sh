#!/bin/bash

# Define the environment file
ENV_FILE=".env"

# Define the container name
CONTAINER_NAME="tx-grpc-server"

# Start the output line
OUTPUT_LINE="container-env-var-updates: "

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
    VALUE=${PARTS[1]}

    # Add the variable to the output line
    OUTPUT_LINE="${OUTPUT_LINE}container=$CONTAINER_NAME,name=$NAME,value=\$$NAME,"
done < "$ENV_FILE"

# Remove the trailing comma
OUTPUT_LINE=${OUTPUT_LINE%?}

# Print the output line
echo "$OUTPUT_LINE"