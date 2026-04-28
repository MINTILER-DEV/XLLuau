const path = require("path");
const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("xluau.restartServer", async () => {
      await restartClient(context);
    })
  );

  startClient(context);
}

async function deactivate() {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

function startClient(context) {
  const command = resolveServerCommand();
  const serverOptions = {
    command,
    transport: TransportKind.stdio
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "xluau" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.xl")
    }
  };

  client = new LanguageClient(
    "xluau",
    "XLuau Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push(client.start());
}

async function restartClient(context) {
  if (client) {
    await client.stop();
    client = undefined;
  }
  startClient(context);
}

function resolveServerCommand() {
  const configured = vscode.workspace
    .getConfiguration("xluau")
    .get("server.path");
  if (configured && configured.trim().length > 0) {
    return configured;
  }

  const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri?.fsPath;
  if (workspaceFolder) {
    const candidate = process.platform === "win32"
      ? path.join(workspaceFolder, "target", "debug", "xluau-lsp.exe")
      : path.join(workspaceFolder, "target", "debug", "xluau-lsp");
    return candidate;
  }

  return "xluau-lsp";
}

module.exports = {
  activate,
  deactivate
};
