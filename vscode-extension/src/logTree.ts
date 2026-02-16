import * as vscode from 'vscode';

interface LogEntry {
    timestamp: string;
    functionName: string;
    method?: string;
    path?: string;
    statusCode: number;
    duration?: number;
    body?: string;
    error?: string;
}

class LogItem extends vscode.TreeItem {
    constructor(public readonly entry: LogEntry) {
        const icon = entry.statusCode >= 400 ? '$(error)' : '$(check)';
        const dur = entry.duration ? `${entry.duration}ms` : '';
        const label = `${entry.functionName} → ${entry.statusCode} ${dur}`;
        super(label, vscode.TreeItemCollapsibleState.None);

        this.description = entry.method && entry.path
            ? `${entry.method} ${entry.path}`
            : entry.timestamp;

        this.tooltip = new vscode.MarkdownString([
            `**${entry.functionName}**`,
            '',
            `- Status: ${entry.statusCode}`,
            entry.duration ? `- Duration: ${entry.duration}ms` : '',
            entry.method ? `- ${entry.method} ${entry.path}` : '',
            `- Time: ${entry.timestamp}`,
            entry.error ? `\n**Error:** ${entry.error}` : '',
        ].filter(Boolean).join('\n'));

        this.iconPath = new vscode.ThemeIcon(
            entry.statusCode >= 400 ? 'error' : 'pass',
            entry.statusCode >= 400
                ? new vscode.ThemeColor('testing.iconFailed')
                : new vscode.ThemeColor('testing.iconPassed')
        );
    }
}

export class LogTreeProvider implements vscode.TreeDataProvider<LogItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<LogItem | undefined>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private logs: LogEntry[] = [];
    private filter: string | null = null;
    private maxLogs = 100;

    getTreeItem(element: LogItem): vscode.TreeItem {
        return element;
    }

    getChildren(): LogItem[] {
        const filtered = this.filter
            ? this.logs.filter(l => l.functionName === this.filter)
            : this.logs;
        return filtered.map(e => new LogItem(e));
    }

    addInvocation(functionName: string, result: { statusCode: number; body: string; duration?: number; error?: string }) {
        this.logs.unshift({
            timestamp: new Date().toISOString(),
            functionName,
            statusCode: result.statusCode,
            duration: result.duration,
            body: result.body,
            error: result.error,
        });
        if (this.logs.length > this.maxLogs) this.logs.pop();
        this._onDidChangeTreeData.fire(undefined);
    }

    /**
     * Parse a structured JSON log line from the lambdaform server.
     */
    addLogLine(line: string) {
        try {
            const parsed = JSON.parse(line);
            if (parsed.function && parsed.status !== undefined) {
                this.logs.unshift({
                    timestamp: parsed.timestamp || new Date().toISOString(),
                    functionName: parsed.function,
                    method: parsed.method,
                    path: parsed.path,
                    statusCode: parsed.status,
                    duration: parsed.duration_ms,
                });
                if (this.logs.length > this.maxLogs) this.logs.pop();
                this._onDidChangeTreeData.fire(undefined);
            }
        } catch {
            // Not JSON, ignore
        }
    }

    filterByFunction(name: string) {
        this.filter = this.filter === name ? null : name;
        this._onDidChangeTreeData.fire(undefined);
        vscode.window.showInformationMessage(
            this.filter ? `Showing logs for ${name}` : 'Showing all logs'
        );
    }

    clear() {
        this.logs = [];
        this.filter = null;
        this._onDidChangeTreeData.fire(undefined);
    }
}
