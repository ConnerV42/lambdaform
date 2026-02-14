package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"

	"github.com/aws/aws-lambda-go/events"
	"github.com/aws/aws-lambda-go/lambda"
)

func handler(ctx context.Context, request events.APIGatewayProxyRequest) (events.APIGatewayProxyResponse, error) {
	greeting := os.Getenv("GREETING")
	if greeting == "" {
		greeting = "Hello from Go"
	}

	name := request.PathParameters["name"]

	var message string
	if name != "" {
		message = fmt.Sprintf("%s, %s!", greeting, name)
	} else {
		message = greeting
	}

	body, _ := json.Marshal(map[string]string{
		"message": message,
	})

	return events.APIGatewayProxyResponse{
		StatusCode: 200,
		Headers: map[string]string{
			"Content-Type": "application/json",
		},
		Body: string(body),
	}, nil
}

func main() {
	lambda.Start(handler)
}
