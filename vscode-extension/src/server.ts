import * as vscode from 'vscode';
import { spawn, ChildProcess } from 'child_process';

export class LambdaformServer {
    private process: ChildProcess | null = null;
    private outputChannel: vscode.OutputChannel;
    private logCallbacks: ((line: string) => void)[] = [];
    private statusCallbacks: ((running: boolean) => void)[] = [];

    constructor(private config: vscode.WorkspaceConfiguration) {
        this.outputChannel = vscode.window.createOutputChannel('Lambdaform');
    }

    async start() {
        if (this.process) {
            vscode.window.showWarningMessage('Lambdaform server is already running');
            return;
        }

        const binary = this.config.get<string>('binaryPath') || 'lambdaform';
        const port = this.config.get<number>('port') || 3000;
        const verbose = this.config.get<boolean>('verbose') || false;
        const jsonLog = this.config.get<boolean>('jsonLog') !== false; // default true
        const terraformDir = this.config.get<string>('terraformDir') || '.';
        const varFiles = this.config.get<string[]>('varFiles') || [];

        const args = ['start', '--port', String(port), '--dir', terraformDir];
        if (verbose) args.push('--verbose');
        if (jsonLog) args.push('--json-log');
        for (const vf of varFiles) {
            args.push('--var-file', vf);
        }

        const ws = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

        this.outputChannel.show(true);
        this.outputChannel.appendLine(`Starting: ${binary} ${args.join(' ')}`);

        this.process = spawn(binary, args, {
            cwd: ws,
            env: { ...process.env },
        });

        this.process.stdout?.on('data', (data: Buffer) => {
            const text = data.toString();
            this.outputChannel.append(text);
            for (const line of text.split('\n').filter(Boolean)) {
                this.logCallbacks.forEach(cb => cb(line));
            }
        });

        this.process.stderr?.on('data', (data: Buffer) => {
            this.outputChannel.append(data.toString());
        });

        this.process.on('exit', (code) => {
            this.outputChannel.appendLine(`\nServer exited with code ${code}`);
            this.process = null;
            this.statusCallbacks.forEach(cb => cb(false));
        });

        this.statusCallbacks.forEach(cb => cb(true));
        vscode.window.showInformationMessage(`Lambdaform server started on port ${port}`);
    }

    stop() {
        if (this.process) {
            this.process.kill('SIGINT');
            this.process = null;
            this.statusCallbacks.forEach(cb => cb(false));
            vscode.window.showInformationMessage('Lambdaform server stopped');
        }
    }

    onLog(callback: (line: string) => void) {
        this.logCallbacks.push(callback);
    }

    onStatusChange(callback: (running: boolean) => void) {
        this.statusCallbacks.push(callback);
    }
}
