"use strict";

var { ExtensionCommon } = ChromeUtils.importESModule("resource://gre/modules/ExtensionCommon.sys.mjs");
var { MailServices } = ChromeUtils.importESModule("resource:///modules/MailServices.sys.mjs");
var { getFolder } = ChromeUtils.importESModule("resource:///modules/ExtensionAccounts.sys.mjs");
var { setTimeout, clearTimeout } = ChromeUtils.importESModule("resource://gre/modules/Timer.sys.mjs");

this.megamailSync = class extends ExtensionCommon.ExtensionAPI {
  getAPI(context) {
    const discoveries = new Set();
    const waitForUrl = (start, timeoutMessage, onStop = null) => new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(timeoutMessage)), 75000);
      const listener = {
        QueryInterface: ChromeUtils.generateQI(["nsIUrlListener"]),
        OnStartRunningUrl() {},
        OnStopRunningUrl(url, status) {
          clearTimeout(timer);
          try {
            if (onStop) onStop(url, status);
          } catch (error) {
            reject(error);
            return;
          }
          if (Components.isSuccessCode(status)) resolve();
          else reject(new Error("Thunderbird mail operation failed (" + status + ")."));
        }
      };
      try {
        start(listener);
      } catch (error) {
        clearTimeout(timer);
        reject(error);
      }
    });
    return {
      megamailSync: {
        async shutdown() {
          setTimeout(() => {
            try {
              Services.startup.quit(Components.interfaces.nsIAppStartup.eAttemptQuit);
            } catch (_) {
              // Native-host disconnect still releases the extension and lets
              // the owning app use its bounded child-process fallback.
            }
          }, 50);
          return { stopping: true };
        },
        async discoverFolders(accountId) {
          const key = String(accountId);
          if (discoveries.has(key)) return { discovered: true, cached: true };
          const account = MailServices.accounts.getAccount(key);
          if (!account || account.incomingServer.type !== "imap") {
            throw new Error("Thunderbird could not resolve this IMAP account.");
          }
          Services.io.offline = false;
          const serverListener = account.incomingServer.QueryInterface(Components.interfaces.nsIUrlListener);
          await waitForUrl(
            listener => MailServices.imap.discoverAllFolders(account.incomingServer.rootFolder, listener, null),
            "Thunderbird folder discovery timed out. Check the account connection and try again.",
            (url, status) => serverListener.OnStopRunningUrl(url, status)
          );
          discoveries.add(key);
          return { discovered: true, cached: false };
        },
        async getNewMessages(folderId) {
          let folder;
          try {
            ({ folder } = getFolder(String(folderId)));
          } catch (_) {
            throw new Error("Thunderbird could not resolve the selected mail folder.");
          }
          if (!folder.server || folder.server.type === "none") {
            return { synced: false, local: true };
          }
          if (folder.server.type !== "imap") {
            throw new Error("MegaMail currently synchronizes IMAP folders only.");
          }
          Services.io.offline = false;
          await waitForUrl(
            listener => folder.QueryInterface(Components.interfaces.nsIMsgImapMailFolder)
              .updateFolderWithListener(null, listener),
            "Thunderbird folder sync timed out. Check the connection and try again."
          );
          return { synced: true, local: false };
        }
      }
    };
  }
};
