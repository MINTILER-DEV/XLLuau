const fs = require("fs");
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

  const folders = vscode.workspace.workspaceFolders ?? [];
  for (const folder of folders) {
    const resolved = findWorkspaceServer(folder.uri.fsPath);
    if (resolved) {
      return resolved;
    }
  }

  return "xluau-lsp";
}

function findWorkspaceServer(startFolder) {
  for (const folder of ancestorFolders(startFolder)) {
    const debugCandidate = serverPathFor(folder, "debug");
    if (fs.existsSync(debugCandidate)) {
      return debugCandidate;
    }

    const releaseCandidate = serverPathFor(folder, "release");
    if (fs.existsSync(releaseCandidate)) {
      return releaseCandidate;
    }
  }

  return null;
}

function ancestorFolders(startFolder) {
  const folders = [];
  let current = path.resolve(startFolder);

  while (true) {
    folders.push(current);
    const parent = path.dirname(current);
    if (parent === current) {
      break;
    }
    current = parent;
  }

  return folders;
}

function serverPathFor(root, profile) {
  const binary = process.platform === "win32" ? "xluau-lsp.exe" : "xluau-lsp";
  return path.join(root, "target", profile, binary);
}

module.exports = {
  activate,
  deactivate
};
