import * as vscode from 'vscode';
import { LambdaformClient, LambdaFunction } from './client';

export class FunctionItem extends vscode.TreeItem {
    constructor(
        public readonly functionName: string,
        public readonly func: LambdaFunction
    ) {
        super(functionName, vscode.TreeItemCollapsibleState.None);
        this.contextValue = 'function';
        this.description = `${func.runtime} · ${func.handler}`;
        this.tooltip = new vscode.MarkdownString([
            `**${functionName}**`,
            '',
            `| Property | Value |`,
            `|----------|-------|`,
            `| Runtime | \`${func.runtime}\` |`,
            `| Handler | \`${func.handler}\` |`,
            func.timeout ? `| Timeout | ${func.timeout}s |` : '',
            func.memorySize ? `| Memory | ${func.memorySize}MB |` : '',
            func.source ? `| Source | \`${func.source}\` |` : '',
        ].filter(Boolean).join('\n'));
        this.iconPath = new vscode.ThemeIcon('symbol-function');
    }
}

export class FunctionTreeProvider implements vscode.TreeDataProvider<FunctionItem> {
    private _onDidChangeTreeData = new vscode.EventEmitter<FunctionItem | undefined>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private functions: LambdaFunction[] = [];

    constructor(
        private client: LambdaformClient,
        private config: vscode.WorkspaceConfiguration
    ) {}

    refresh(): void {
        this.client.listFunctions().then(funcs => {
            this.functions = funcs;
            this._onDidChangeTreeData.fire(undefined);
        });
    }

    getTreeItem(element: FunctionItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: FunctionItem): Promise<FunctionItem[]> {
        if (element) return [];

        if (this.functions.length === 0) {
            await this.client.listFunctions().then(funcs => {
                this.functions = funcs;
            });
        }

        return this.functions.map(f => new FunctionItem(f.name, f));
    }
}
