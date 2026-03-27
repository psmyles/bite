// ImgplexBatchTool.cs
// Drop into Assets/Editor/ in your Unity project.
//
// Setup:
//   1. Install imgplex from the Windows installer (imgplex-Windows-x.x.x-Setup.exe).
//      The installer places imgplex-cli.exe in %LOCALAPPDATA%\Programs\imgplex\ and
//      adds that folder to your PATH automatically.
//   2. Save a workflow from imgplex: File > Save Workflow  →  produces a .imgplex file
//   3. Right-click any asset > Imgplex > Settings...
//        • imgplex CLI  →  auto-detected; override only if you chose a custom install path
//        • Workflows Folder  →  folder containing your .imgplex files
//   4. Select image assets, right-click > Imgplex > Run Script...
//      A small window lists every workflow — click one to process the selected images.

using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using UnityEditor;
using UnityEngine;

public static class ImgplexBatchTool
{
    // ── Persisted settings ────────────────────────────────────────────────────
    private const string KeyWorkflowsFolder = "ImgplexBatch_WorkflowsFolder";
    private const string KeyCliExe          = "ImgplexBatch_CliExe";

    private static string WorkflowsFolder
    {
        get => EditorPrefs.GetString(KeyWorkflowsFolder, "");
        set => EditorPrefs.SetString(KeyWorkflowsFolder, value);
    }

    /// <summary>
    /// Optional override path to imgplex-cli.exe. When empty, auto-detection is used.
    /// </summary>
    private static string CliExe
    {
        get => EditorPrefs.GetString(KeyCliExe, "");
        set => EditorPrefs.SetString(KeyCliExe, value);
    }

    // ── CLI resolution ────────────────────────────────────────────────────────
    /// <summary>
    /// Returns the full path to imgplex-cli.exe, or null if not found.
    /// Priority: (1) user override, (2) default installer location.
    /// </summary>
    private static string ResolveCliExe()
    {
        // User has set an explicit path — use it if valid.
        if (!string.IsNullOrEmpty(CliExe) && File.Exists(CliExe))
            return CliExe;

        // Default per-user install location used by the NSIS installer.
        string defaultPath = Path.Combine(
            System.Environment.GetFolderPath(System.Environment.SpecialFolder.LocalApplicationData),
            "Programs", "imgplex", "imgplex-cli.exe");

        if (File.Exists(defaultPath))
            return defaultPath;

        return null;
    }

    // ── Context menu: run a workflow ──────────────────────────────────────────
    [MenuItem("Assets/Imgplex/Run Script \u2026", false, 1200)]
    private static void ShowScriptMenu()
    {
        if (!EnsureSettings()) return;

        var      imagePaths = GetSelectedImagePaths();   // capture before window opens
        string[] workflows  = Directory.GetFiles(WorkflowsFolder, "*.imgplex");

        if (workflows.Length == 0)
        {
            EditorUtility.DisplayDialog("Imgplex",
                $"No .imgplex files found in:\n{WorkflowsFolder}", "OK");
            return;
        }

        WorkflowPickerWindow.Open(workflows, imagePaths);
    }

    [MenuItem("Assets/Imgplex/Run Script \u2026", true)]
    private static bool ValidateShowScriptMenu() => GetSelectedImagePaths().Count > 0;

    // ── Context menu: settings ────────────────────────────────────────────────
    [MenuItem("Assets/Imgplex/Settings\u2026", false, 1210)]
    private static void OpenSettings() => SettingsWindow.Open();

    // ── Ensure CLI + workflow folder are configured ───────────────────────────
    private static bool EnsureSettings()
    {
        bool cliOk    = ResolveCliExe() != null;
        bool folderOk = !string.IsNullOrEmpty(WorkflowsFolder) && Directory.Exists(WorkflowsFolder);

        if (!cliOk || !folderOk)
        {
            EditorUtility.DisplayDialog("Imgplex — Setup Required",
                "Before running workflows, configure:\n\n" +
                (cliOk    ? "" : "  • imgplex is not installed or the CLI path is wrong.\n" +
                                 "    Install from imgplex-Windows-x.x.x-Setup.exe or set the\n" +
                                 "    path manually via Assets > Imgplex > Settings...\n") +
                (folderOk ? "" : "  • Workflows Folder\n"),
                "OK");
            if (!folderOk) SettingsWindow.Open();
            return false;
        }
        return true;
    }

    // ── Workflow execution ────────────────────────────────────────────────────
    internal static void RunWorkflow(string workflowPath, List<string> imagePaths)
    {
        if (imagePaths.Count == 0)
        {
            EditorUtility.DisplayDialog("Imgplex", "No image assets were selected.", "OK");
            return;
        }

        string resolvedCli = ResolveCliExe();
        if (resolvedCli == null)
        {
            EditorUtility.DisplayDialog("Imgplex",
                "imgplex-cli.exe not found.\n\nInstall imgplex from the Windows installer, " +
                "or set the path manually via Assets > Imgplex > Settings...", "OK");
            return;
        }

        string wfName    = Path.GetFileNameWithoutExtension(workflowPath);
        int    processed = 0;
        var    errors    = new List<string>();

        var byFolder = new Dictionary<string, List<string>>();
        foreach (string abs in imagePaths)
        {
            string dir = Path.GetDirectoryName(abs)!;
            if (!byFolder.ContainsKey(dir)) byFolder[dir] = new List<string>();
            byFolder[dir].Add(abs);
        }

        foreach (var kvp in byFolder)
        {
            string outputDir = kvp.Key;
            string tempDir   = Path.Combine(
                Path.GetTempPath(), $"imgplex_{System.Guid.NewGuid():N}");
            Directory.CreateDirectory(tempDir);

            try
            {
                foreach (string src in kvp.Value)
                    File.Copy(src, Path.Combine(tempDir, Path.GetFileName(src)), overwrite: true);

                var psi = new ProcessStartInfo
                {
                    FileName               = resolvedCli,
                    Arguments              = $"run \"{workflowPath}\" --input \"{tempDir}\" --output \"{outputDir}\" --overwrite",
                    UseShellExecute        = false,
                    RedirectStandardOutput = true,
                    RedirectStandardError  = true,
                    CreateNoWindow         = true,
                };

                using var proc   = Process.Start(psi)!;
                string    stdout = proc.StandardOutput.ReadToEnd();
                string    stderr = proc.StandardError.ReadToEnd();
                proc.WaitForExit();

                if (proc.ExitCode != 0)
                {
                    string detail = string.IsNullOrEmpty(stderr) ? stdout : stderr;
                    errors.Add($"[{outputDir}] Exit {proc.ExitCode}: {detail.Trim()}");
                }
                else
                {
                    processed += kvp.Value.Count;
                }
            }
            catch (System.Exception ex)
            {
                errors.Add($"[{outputDir}] {ex.Message}");
            }
            finally
            {
                try { Directory.Delete(tempDir, recursive: true); } catch { /* non-fatal */ }
            }
        }

        AssetDatabase.Refresh();

        if (errors.Count > 0)
            UnityEngine.Debug.LogError(
                $"[Imgplex] '{wfName}' errors:\n{string.Join("\n", errors)}");
        else
            UnityEngine.Debug.Log(
                $"[Imgplex] '{wfName}' processed {processed} image(s).");
    }

    // ── Helpers ───────────────────────────────────────────────────────────────
    private static List<string> GetSelectedImagePaths()
    {
        var result = new List<string>();
        foreach (var obj in Selection.objects)
        {
            if (obj is not (Texture2D or Sprite)) continue;
            string ap = AssetDatabase.GetAssetPath(obj);
            if (!string.IsNullOrEmpty(ap))
                result.Add(Path.GetFullPath(ap));
        }
        return result;
    }

    // ── Settings window ───────────────────────────────────────────────────────
    private sealed class SettingsWindow : EditorWindow
    {
        internal static void Open()
        {
            var win          = GetWindow<SettingsWindow>(true, "Imgplex Settings");
            win.minSize      = new Vector2(480f, 150f);
            win.maxSize      = new Vector2(760f, 150f);
        }

        private void OnGUI()
        {
            EditorGUILayout.Space(6);

            // CLI executable row
            string detected = ResolveCliExe();
            string label    = detected != null
                ? $"imgplex CLI  (auto-detected: {detected})"
                : "imgplex CLI  (not found — set path manually)";
            EditorGUILayout.LabelField(label, EditorStyles.miniBoldLabel);
            EditorGUILayout.BeginHorizontal();
            CliExe = EditorGUILayout.TextField(CliExe, GUILayout.ExpandWidth(true));
            if (GUILayout.Button("\u2026", GUILayout.Width(28)))
            {
                string p = EditorUtility.OpenFilePanel("Locate imgplex-cli.exe",
                    string.IsNullOrEmpty(CliExe) ? detected ?? "" : CliExe, "exe");
                if (!string.IsNullOrEmpty(p)) CliExe = p;
            }
            EditorGUILayout.EndHorizontal();

            EditorGUILayout.Space(6);

            // Workflows folder row
            EditorGUILayout.LabelField("Workflows Folder  (.imgplex files)", EditorStyles.miniBoldLabel);
            EditorGUILayout.BeginHorizontal();
            WorkflowsFolder = EditorGUILayout.TextField(WorkflowsFolder);
            if (GUILayout.Button("\u2026", GUILayout.Width(28)))
            {
                string p = EditorUtility.OpenFolderPanel("Select Workflows Folder", WorkflowsFolder, "");
                if (!string.IsNullOrEmpty(p)) WorkflowsFolder = p;
            }
            EditorGUILayout.EndHorizontal();

            EditorGUILayout.Space(8);
            if (GUILayout.Button("Done", GUILayout.Height(24))) Close();
        }
    }

    // ── Workflow picker window ────────────────────────────────────────────────
    private sealed class WorkflowPickerWindow : EditorWindow
    {
        private string[]     _workflows;
        private List<string> _imagePaths;
        private Vector2      _scroll;

        internal static void Open(string[] workflows, List<string> imagePaths)
        {
            var win          = CreateInstance<WorkflowPickerWindow>();
            win.titleContent = new GUIContent("Imgplex \u2014 Select Workflow");
            win._workflows   = workflows;
            win._imagePaths  = imagePaths;
            float h          = Mathf.Clamp(workflows.Length * 28f + 16f, 60f, 360f);
            win.minSize      = new Vector2(260f, h);
            win.maxSize      = new Vector2(480f, h);
            win.ShowUtility();
        }

        private void OnGUI()
        {
            _scroll = EditorGUILayout.BeginScrollView(_scroll);
            foreach (string wf in _workflows)
            {
                if (GUILayout.Button(Path.GetFileNameWithoutExtension(wf), GUILayout.Height(26)))
                {
                    string       captured = wf;
                    List<string> images   = _imagePaths;
                    Close();
                    RunWorkflow(captured, images);
                    return;
                }
            }
            EditorGUILayout.EndScrollView();
        }
    }
}
