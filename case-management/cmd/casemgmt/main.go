package main

import (
	"context"
	"io/fs"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/siem-lite/case-management/internal/schema"
	"github.com/siem-lite/case-management/internal/server"
	"github.com/siem-lite/case-management/internal/store"
	siemweb "github.com/siem-lite/case-management/web"
)

func main() {
	log := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelInfo}))

	dsn := strings.TrimSpace(os.Getenv("CASEMGMT_DATABASE_URL"))
	if dsn == "" {
		log.Error("CASEMGMT_DATABASE_URL is required")
		os.Exit(1)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	db, err := store.NewPostgres(ctx, dsn)
	if err != nil {
		log.Error("postgres", "err", err)
		os.Exit(1)
	}
	defer db.Close()

	if err := db.Migrate(ctx, schema.InitSQL); err != nil {
		log.Error("migrate", "err", err)
		os.Exit(1)
	}

	srv := server.New(db)
	r := srv.Router()

	sub, err := fs.Sub(siemweb.Files, "dist")
	if err == nil {
		r.NotFound(spaHandler(sub))
	}

	addr := ":8088"
	if v := strings.TrimSpace(os.Getenv("CASEMGMT_HTTP_ADDR")); v != "" {
		addr = v
	}

	httpSrv := &http.Server{
		Addr:              addr,
		Handler:           r,
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       60 * time.Second,
		WriteTimeout:      60 * time.Second,
	}

	go func() {
		log.Info("case-management listening", "addr", addr)
		if err := httpSrv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Error("http", "err", err)
			os.Exit(1)
		}
	}()

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, syscall.SIGINT, syscall.SIGTERM)
	<-sig
	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer shutdownCancel()
	_ = httpSrv.Shutdown(shutdownCtx)
}

func spaHandler(dist fs.FS) http.HandlerFunc {
	fileServer := http.FileServer(http.FS(dist))
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			w.WriteHeader(http.StatusNotFound)
			return
		}
		path := strings.TrimPrefix(r.URL.Path, "/")
		if path == "" || !strings.Contains(path, ".") {
			r2 := r.Clone(r.Context())
			r2.URL.Path = "/"
			fileServer.ServeHTTP(w, r2)
			return
		}
		fileServer.ServeHTTP(w, r)
	}
}
