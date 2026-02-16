import * as vscode from 'vscode';
import { FunctionTreeProvider, FunctionItem } from './functionTree';
import { LogTreeProvider } from './logTree';
import { LambdaformServer } from './server';
import { LambdaformClient } from './client';

let server: LambdaformServer;
let client: LambdaformClient;
let functionTree: FunctionTreeProvider;
let logTree: LogTreeProvider;

export function activate(context: vscode.ExtensionContext) {
    const config = vscode.workspace.getConfiguration('lambdaform');

    server = new LambdaformServer(config);
    client = new LambdaformClient(config);
    functionTree = new FunctionTreeProvider(client, config);
    logTree = new LogTreeProvider();

    // Register tree views
    vscode.window.registerTreeDataProvider('lambdaformFunctions', functionTree);
    vscode.window.registerTreeDataProvider('lambdaformLogs', logTree);

    // Server lifecycle
    context.subscriptions.push(
        vscode.commands.registerCommand('lambdaform.start', async () => {
            await server.start();
            vscode.commands.executeCommand('setContext', 'lambdaform.serverRunning', true);
            // Parse structured logs for the log viewer
            server.onLog((line) => logTree.addLogLine(line));
            // Refresh function list after server starts
            setTimeout(() => functionTree.refresh(), 1500);
        }),

        vscode.commands.registerCommand('lambdaform.stop', () => {
            server.stop();
            vscode.commands.executeCommand('setContext', 'lambdaform.serverRunning', false);
        }),

        // Function operations
        vscode.commands.registerCommand('lambdaform.refresh', () => {
            functionTree.refresh();
        }),

        vscode.commands.registerCommand('lambdaform.invoke', async (item?: FunctionItem) => {
            const funcName = item?.functionName ?? await pickFunction();
            if (!funcName) return;
            const result = await client.invoke(funcName, '{}');
            logTree.addInvocation(funcName, result);
            showInvocationResult(funcName, result);
        }),

        vscode.commands.registerCommand('lambdaform.invokeWithPayload', async (item?: FunctionItem) => {
            const funcName = item?.functionName ?? await pickFunction();
            if (!funcName) return;
            const payload = await vscode.window.showInputBox({
                prompt: `JSON payload for ${funcName}`,
                value: '{}',
                validateInput: (v) => {
                    try { JSON.parse(v); return null; } catch { return 'Invalid JSON'; }
                }
            });
            if (payload === undefined) return;
            const result = await client.invoke(funcName, payload);
            logTree.addInvocation(funcName, result);
            showInvocationResult(funcName, result);
        }),

        vscode.commands.registerCommand('lambdaform.viewLogs', async (item?: FunctionItem) => {
            const funcName = item?.functionName ?? await pickFunction();
            if (!funcName) return;
            logTree.filterByFunction(funcName);
        }),

        vscode.commands.registerCommand('lambdaform.clearLogs', () => {
            logTree.clear();
        }),

        vscode.commands.registerCommand('lambdaform.openConfig', async () => {
            const files = await vscode.workspace.findFiles(
                '{lambdaform.yaml,lambdaform.yml}', null, 1
            );
            if (files.length > 0) {
                vscode.window.showTextDocument(files[0]);
            } else {
                vscode.window.showInformationMessage('No lambdaform.yaml found. Run "lambdaform init" to create one.');
            }
        })
    );

    // Auto-start if configured
    if (config.get<boolean>('autoStart')) {
        vscode.commands.executeCommand('lambdaform.start');
    }

    // Refresh on Terraform file changes
    const watcher = vscode.workspace.createFileSystemWatcher('**/*.tf');
    watcher.onDidChange(() => functionTree.refresh());
    watcher.onDidCreate(() => functionTree.refresh());
    watcher.onDidDelete(() => functionTree.refresh());
    context.subscriptions.push(watcher);

    // Status bar
    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBar.text = '$(cloud) Lambdaform';
    statusBar.command = 'lambdaform.start';
    statusBar.tooltip = 'Click to start Lambdaform server';
    statusBar.show();
    context.subscriptions.push(statusBar);

    server.onStatusChange((running) => {
        statusBar.text = running ? '$(cloud) Lambdaform ●' : '$(cloud) Lambdaform';
        statusBar.command = running ? 'lambdaform.stop' : 'lambdaform.start';
        statusBar.tooltip = running ? 'Lambdaform running — click to stop' : 'Click to start Lambdaform server';
    });
}

export function deactivate() {
    server?.stop();
}

async function pickFunction(): Promise<string | undefined> {
    const functions = await client.listFunctions();
    if (functions.length === 0) {
        vscode.window.showWarningMessage('No Lambda functions found. Is Lambdaform running?');
        return;
    }
    return vscode.window.showQuickPick(functions.map(f => f.name), {
        placeHolder: 'Select a Lambda function'
    });
}

function showInvocationResult(funcName: string, result: { statusCode: number; body: string; duration?: number }) {
    const doc = vscode.workspace.openTextDocument({
        content: [
            `// Invocation: ${funcName}`,
            `// Status: ${result.statusCode}`,
            result.duration ? `// Duration: ${result.duration}ms` : '',
            '',
            result.body
        ].filter(Boolean).join('\n'),
        language: 'json'
    });
    doc.then(d => vscode.window.showTextDocument(d, { preview: true, viewColumn: vscode.ViewColumn.Beside }));
}
