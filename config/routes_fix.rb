# 라우트 설정 수정 가이드

# config/routes.rb 파일에 다음 내용을 추가하세요:

Rails.application.routes.draw do
  # ... existing code ...
  
  namespace :api do
    # API 라우트 (v1 없이)
    resources :tasks do
      collection do
        post :generate
      end
    end
    
    # holidays 라우트
    get '/holidays/:year/:month', to: 'holidays#index'
  end
  
  # ... existing code ...
end