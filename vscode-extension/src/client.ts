import * as vscode from 'vscode';
import { execFile } from 'child_process';
import { promisify } from 'util';
import * as http from 'http';

const execFileAsync = promisify(execFile);

export interface LambdaFunction {
    name: string;
    runtime: string;
    handler: string;
    source: string; // Terraform file path
    timeout?: number;
    memorySize?: number;
}

export interface InvocationResult {
    statusCode: number;
    body: string;
    duration?: number;
    error?: string;
}

export class LambdaformClient {
    constructor(private config: vscode.WorkspaceConfiguration) {}

    private getBinary(): string {
        return this.config.get<string>('binaryPath') || 'lambdaform';
    }

    private getDir(): string {
        const dir = this.config.get<string>('terraformDir') || '.';
        const ws = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (ws && dir !== '.') {
            return `${ws}/${dir}`;
        }
        return ws || dir;
    }

    /**
     * List Lambda functions by running `lambdaform config --json`
     */
    async listFunctions(): Promise<LambdaFunction[]> {
        try {
            const { stdout } = await execFileAsync(this.getBinary(), [
                'config', '--dir', this.getDir()
            ], { timeout: 10000 });

            return this.parseFunctionList(stdout);
        } catch (err: any) {
            vscode.window.showErrorMessage(`Lambdaform: ${err.message}`);
            return [];
        }
    }

    /**
     * Parse the output of `lambdaform config` into function objects.
     * The config command outputs function details in a structured format.
     */
    private parseFunctionList(output: string): LambdaFunction[] {
        const functions: LambdaFunction[] = [];
        const lines = output.split('\n');

        let current: Partial<LambdaFunction> | null = null;

        for (const line of lines) {
            const nameMatch = line.match(/Function:\s+(\S+)/);
            if (nameMatch) {
                if (current?.name) {
                    functions.push(current as LambdaFunction);
                }
                current = { name: nameMatch[1], runtime: 'unknown', handler: 'unknown', source: '' };
                continue;
            }
            if (!current) continue;

            const runtimeMatch = line.match(/Runtime:\s+(\S+)/);
            if (runtimeMatch) current.runtime = runtimeMatch[1];

            const handlerMatch = line.match(/Handler:\s+(\S+)/);
            if (handlerMatch) current.handler = handlerMatch[1];

            const sourceMatch = line.match(/Source:\s+(.+)/);
            if (sourceMatch) current.source = sourceMatch[1].trim();

            const timeoutMatch = line.match(/Timeout:\s+(\d+)/);
            if (timeoutMatch) current.timeout = parseInt(timeoutMatch[1]);

            const memoryMatch = line.match(/Memory:\s+(\d+)/);
            if (memoryMatch) current.memorySize = parseInt(memoryMatch[1]);
        }
        if (current?.name) {
            functions.push(current as LambdaFunction);
        }

        return functions;
    }

    /**
     * Invoke a function via the running server's invoke endpoint.
     * Uses `lambdaform invoke <name> --json <payload>`.
     */
    async invoke(functionName: string, payload: string): Promise<InvocationResult> {
        try {
            const port = this.config.get<number>('port') || 3000;
            const result = await this.httpPost(
                `http://127.0.0.1:${port}/__lambdaform/invoke/${functionName}`,
                payload
            );
            return result;
        } catch {
            // Fallback to CLI invoke
            try {
                const { stdout, stderr } = await execFileAsync(this.getBinary(), [
                    'invoke', functionName, '--json', payload, '--dir', this.getDir()
                ], { timeout: 30000 });
                return {
                    statusCode: 200,
                    body: stdout,
                    error: stderr || undefined
                };
            } catch (err: any) {
                return {
                    statusCode: 500,
                    body: '',
                    error: err.message
                };
            }
        }
    }

    private httpPost(url: string, body: string): Promise<InvocationResult> {
        return new Promise((resolve, reject) => {
            const parsed = new URL(url);
            const startTime = Date.now();
            const req = http.request({
                hostname: parsed.hostname,
                port: parsed.port,
                path: parsed.pathname,
                method: 'POST',
                headers: { 'Content-Type': 'application/json' }
            }, (res) => {
                let data = '';
                res.on('data', (chunk) => data += chunk);
                res.on('end', () => {
                    resolve({
                        statusCode: res.statusCode || 200,
                        body: data,
                        duration: Date.now() - startTime
                    });
                });
            });
            req.on('error', reject);
            req.write(body);
            req.end();
        });
    }
}
