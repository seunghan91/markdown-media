InertiaRails.configure do |config|
  config.version = -> { Digest::MD5.hexdigest(Rails.root.join("public/assets/manifest.json").read) rescue nil }
end
