package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/aws/aws-lambda-go/events"
	"github.com/aws/aws-lambda-go/lambda"
)

func handler(ctx context.Context, request events.APIGatewayProxyRequest) (events.APIGatewayProxyResponse, error) {
	switch request.HTTPMethod {
	case "GET":
		if request.PathParameters["id"] != "" {
			return respond(200, map[string]interface{}{
				"id":        request.PathParameters["id"],
				"language":  "Go",
				"timestamp": time.Now().UTC().Format(time.RFC3339),
			})
		}
		return respond(200, map[string]interface{}{
			"message": "Hello from Go Lambda!",
			"time":    time.Now().UTC().Format(time.RFC3339),
		})
	case "POST":
		var body map[string]interface{}
		if err := json.Unmarshal([]byte(request.Body), &body); err != nil {
			return respond(400, map[string]string{"error": "Invalid JSON"})
		}
		body["id"] = "go-123"
		body["created"] = time.Now().UTC().Format(time.RFC3339)
		return respond(201, body)
	default:
		return respond(405, map[string]string{"error": fmt.Sprintf("Method %s not allowed", request.HTTPMethod)})
	}
}

func respond(status int, body interface{}) (events.APIGatewayProxyResponse, error) {
	b, _ := json.Marshal(body)
	return events.APIGatewayProxyResponse{
		StatusCode: status,
		Headers:    map[string]string{"Content-Type": "application/json"},
		Body:       string(b),
	}, nil
}

func main() {
	lambda.Start(handler)
}
